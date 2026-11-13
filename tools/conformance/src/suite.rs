/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One Independently Reported Case per Numbered Behaviour Rule.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

// Third Party Imports
use futures_util::stream;
use prost::Message;
use tokio::sync::mpsc;
use tonic::{Code, Streaming, transport::Channel};

// Local Imports
use crate::{Error, Result, fixture};
use harmos_protocol::plugin::{
    instance_client::InstanceClient, instance_invoke_request, instance_invoke_response, instance_samples_request,
};
use harmos_protocol::v1::{self as v1, child_response, response};

/* === Constants === */

pub const CASES: &[&str] = &[
    "Get and Catalog",
    "Activation and Started At",
    "Invoke Result and EOF",
    "Invoke Frame Validation",
    "Child Calls and Reordering",
    "Child Failure Preservation",
    "Cancellation and Child Cancel",
    "Structured Failure",
    "Transport Failure",
    "Ticks and Samples",
    "Independent Calls",
    "Device Samples",
    "Demand and Reopen",
    "Samples Frame Validation",
    "Child Correlation Validation",
    "Stop and Cleanup",
    "Schema Fingerprints",
    "Interval Setting",
    "Built-In Clock Fingerprints",
    "Descriptor-Only Registration",
    "Explicit Visibility and Replay Safety",
    "Invocation Resource Isolation",
    "Cooperative Cancellation Reason",
    "Local Private Isolation",
    "Demand-Bounded Context Watch",
    "Local Private Retry Policy",
    "Local Private Attempt Deadline",
    "Local Private Cooperative Cancellation",
    "Host-Mediated Peer Visibility",
    "Wire Operation Identities",
    "Message Ceiling Independent of Transfer Chunks",
];

static INVOCATION: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

type Client = InstanceClient<Channel>;
type Invoke = (mpsc::Sender<harmos_protocol::plugin::InstanceInvokeRequest>, Streaming<harmos_protocol::plugin::InstanceInvokeResponse>);
type Publish = (mpsc::Sender<harmos_protocol::plugin::InstanceSamplesRequest>, Streaming<harmos_protocol::plugin::InstanceSamplesResponse>);

/* === Structs === */

pub struct Case {
    pub rule: usize,
    pub name: &'static str,
    pub error: Option<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn run(address: &str, managed: bool) -> Result<Vec<Case>> {
    let mut results = Vec::new();
    // Keep Stop's Rule Number Stable and Run It After Additive Cases.
    let order = (1..=CASES.len()).filter(|rule| *rule != 16).chain(std::iter::once(16));
    for rule in order {
        let name = CASES[rule - 1];
        let result = tokio::time::timeout(Duration::from_secs(30), case(rule, address, managed)).await;
        let error = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some("Case Hang Guard Expired".into()),
        };
        results.push(Case { rule, name, error });
    }
    Ok(results)
}

/* === Private Functions === */

async fn connect(address: &str) -> Result<(Client, harmos_protocol::plugin::InstanceGetResponse)> {
    let maximum = harmos_protocol::transfer::default_message_bytes().get();
    let mut client = Client::connect(address.to_owned()).await?.max_decoding_message_size(maximum).max_encoding_message_size(maximum);
    let description = describe(&mut client).await?;
    Ok((client, description))
}

async fn describe(client: &mut Client) -> Result<harmos_protocol::plugin::InstanceGetResponse> {
    Ok(client
        .get(harmos_protocol::plugin::InstanceGetRequest {
            revision: 1,
            host: "conformance_host".into(),
            host_version: "1.0.0".into(),
        })
        .await?
        .into_inner())
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition { Ok(()) } else { Err(Error::Invalid(message.into())) }
}

fn request(operation: &str, payload: Vec<u8>) -> harmos_protocol::plugin::InstanceInvokeRequest {
    let invocation = INVOCATION.fetch_add(1, Ordering::Relaxed);
    harmos_protocol::plugin::InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Request(v1::Request {
            operation: operation.into(),
            payload,
            invocation: format!("fixture-{invocation}"),
            attempt: 1,
            scope: "fixture".into(),
        })),
    }
}

async fn invoke(client: &mut Client, operation: &str, payload: Vec<u8>) -> Result<Invoke> {
    let (send, receive) = mpsc::channel(8);
    send.send(request(operation, payload)).await.map_err(|error| Error::Invalid(error.to_string()))?;
    let outgoing = stream::unfold(receive, |mut receive| async { receive.recv().await.map(|frame| (frame, receive)) });
    let incoming = client.invoke(outgoing).await?.into_inner();
    Ok((send, incoming))
}

async fn terminal(input: &mut Streaming<harmos_protocol::plugin::InstanceInvokeResponse>) -> Result<response::Result> {
    let frame = input.message().await?.and_then(|frame| frame.frame);
    let Some(instance_invoke_response::Frame::Response(v1::Response { result: Some(result) })) = frame else {
        return Err(Error::Invalid(format!("Expected Terminal Response, Got {frame:?}")));
    };
    require(input.message().await?.is_none(), "Frames After Terminal Response")?;
    Ok(result)
}

async fn child(
    input: &mut Streaming<harmos_protocol::plugin::InstanceInvokeResponse>, id: u64, operation: &str, payload: &[u8],
) -> Result<()> {
    let frame = input.message().await?.and_then(|frame| frame.frame);
    let Some(instance_invoke_response::Frame::Child(child)) = frame else {
        return Err(Error::Invalid(format!("Expected Child Request, Got {frame:?}")));
    };
    require(
        child.call == id
            && child.operation == operation
            && child.payload == payload
            && child.fingerprint == fixture::fingerprint(operation)
            && child.policy.is_none(),
        "Child Contract or Correlation Differs",
    )
}

async fn send_child(
    send: &mpsc::Sender<harmos_protocol::plugin::InstanceInvokeRequest>, call: u64, result: child_response::Result,
) -> Result<()> {
    send.send(harmos_protocol::plugin::InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Child(v1::ChildResponse { call, result: Some(result) })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))
}

async fn open(client: &mut Client, name: &str) -> Result<Publish> {
    let (send, receive) = mpsc::channel(8);
    send.send(harmos_protocol::plugin::InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Open(v1::SamplesSelection {
            stream: name.into(),
            scope: "fixture".into(),
        })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))?;
    let outgoing = stream::unfold(receive, |mut receive| async { receive.recv().await.map(|frame| (frame, receive)) });
    let incoming = client.samples(outgoing).await?.into_inner();
    Ok((send, incoming))
}

async fn tick(
    send: &mpsc::Sender<harmos_protocol::plugin::InstanceSamplesRequest>,
    input: &mut Streaming<harmos_protocol::plugin::InstanceSamplesResponse>, sequence: u64, seconds: i64,
) -> Result<()> {
    let at = fixture::timestamp(seconds);
    send.send(harmos_protocol::plugin::InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Tick(v1::Tick { at: Some(at) })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))?;
    let samples = input.message().await?.ok_or_else(|| Error::Invalid("Missing Tick Sample".into()))?;
    let expected = v1::Sample {
        at: Some(at),
        row: fixture::payload(&sequence.to_le_bytes()),
        sequence,
    };
    require(samples.samples == [expected], "Tick Time, Row, Sequence or Sample Count Differs")
}

async fn invoke_status(
    result: std::result::Result<tonic::Response<Streaming<harmos_protocol::plugin::InstanceInvokeResponse>>, tonic::Status>, expected: Code,
) -> Result<()> {
    let status = match result {
        Err(status) => status,
        Ok(response) => response.into_inner().message().await.err().ok_or_else(|| Error::Invalid("Expected gRPC Error".into()))?,
    };
    require(status.code() == expected, &format!("Expected {expected:?}, Got {status}"))
}

async fn publish_status(
    result: std::result::Result<tonic::Response<Streaming<harmos_protocol::plugin::InstanceSamplesResponse>>, tonic::Status>,
    expected: Code,
) -> Result<()> {
    let status = match result {
        Err(status) => status,
        Ok(response) => response.into_inner().message().await.err().ok_or_else(|| Error::Invalid("Expected gRPC Error".into()))?,
    };
    require(status.code() == expected, &format!("Expected {expected:?}, Got {status}"))
}

async fn case(rule: usize, address: &str, managed: bool) -> Result<()> {
    let (mut client, description) = connect(address).await?;
    match rule {
        31 => {
            // Echo a Message Larger Than the Storage Chunk Through the Default Carrier.
            let chunk = harmos_protocol::storage::StorageTransferSettings::default().chunk_bytes() as usize;
            let payload = fixture::payload(&vec![0x5a; chunk + harmos_protocol::storage::TransferFraming::Bytes as usize]);
            let (_send, mut input) = invoke(&mut client, "conformance.step.0", payload.clone()).await?;
            require(terminal(&mut input).await? == response::Result::Payload(payload), "Invoke Large Echo: Payload Differs")?;
        }
        30 => {
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Declare: Missing Catalog".into()))?;
            for operation in &catalog.operations {
                harmos_protocol::schema::plugin_operation_identity(&operation.id).map_err(|error| Error::Invalid(error.to_string()))?;
            }
            for (id, visibility) in [
                ("conformance.step.0", v1::Visibility::Public),
                ("example.plugin.v1.Lifecycle/Initialize", v1::Visibility::Protected),
                ("example.plugin.v1.Steps/Execute", v1::Visibility::Public),
            ] {
                let operation = catalog
                    .operations
                    .iter()
                    .find(|operation| operation.id == id)
                    .ok_or_else(|| Error::Invalid(format!("Declare {id}: Missing Operation")))?;
                require(operation.visibility == visibility as i32, "Declare: Wire Operation Visibility Differs")?;
                let payload = fixture::payload(b"wire identity");
                let (send, mut input) = invoke(&mut client, id, payload.clone()).await?;
                if id == "example.plugin.v1.Steps/Execute" {
                    child(&mut input, 1, "conformance.host.echo", &payload).await?;
                    send_child(&send, 1, child_response::Result::Payload(payload.clone())).await?;
                }
                require(terminal(&mut input).await? == response::Result::Payload(payload), "Invoke: Wire Identity Result Differs")?;
            }
            for id in ["example..step.0", "example.plugin.v1.Steps/", "harmos.plugin.v1.Instance/Invoke"] {
                let error =
                    harmos_protocol::schema::plugin_operation_identity(id).expect_err("Register: Invalid Wire Identity Was Admitted");
                require(error.to_string().contains(id), "Register: Identity Refusal Lost Its Action")?;
            }
        }
        1 => {
            require(
                description.id == "conformance"
                    && description.version == "1.0.0"
                    && description.revision == 1
                    && !description.sdk.is_empty()
                    && !description.sdk_version.is_empty(),
                "Plugin Identity Missing",
            )?;
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            let expected = fixture::catalog();
            require(
                expected.operations.iter().all(|entry| catalog.operations.iter().find(|actual| actual.id == entry.id) == Some(entry))
                    && catalog.requires.len() == expected.requires.len()
                    && expected.requires.iter().all(|entry| catalog.requires.iter().find(|actual| actual.id == entry.id) == Some(entry))
                    && catalog.streams.get(..2) == expected.streams.get(..2),
                "Fixed Fixture Catalog Differs",
            )?;
            require(catalog.digest == harmos_protocol::schema::catalog_digest(&catalog), "Catalog Digest Differs")?;
        }
        2 => {
            let first = description;
            drop(client);
            let (_next, second) = connect(address).await?;
            let at = first.started_at.ok_or_else(|| Error::Invalid("Missing Started At".into()))?;
            require(
                at.nanos >= 0 && at.nanos < 1_000_000_000 && (-62_135_596_800..=253_402_300_799).contains(&at.seconds),
                "Invalid Started At",
            )?;
            require(second.started_at == Some(at) && second.catalog == first.catalog, "Reconnect Changed Process Identity or Catalog")?;
        }
        3 => {
            for value in [vec![], vec![0, 255, 1, 128]] {
                let payload = fixture::payload(&value);
                let (send, mut input) = invoke(&mut client, "conformance.step.0", payload.clone()).await?;
                drop(send);
                require(terminal(&mut input).await? == response::Result::Payload(payload), "Echo Changed Protobuf Payload")?;
            }

            // The Author Opts Into Scope Checking; Ordinary Calls Stay Unscoped.
            for scope in ["scope/α", "scope/β", ""] {
                let mut frame = request("conformance.step.0", fixture::payload(b"scope"));
                let Some(instance_invoke_request::Frame::Request(request)) = frame.frame.as_mut() else { unreachable!() };
                request.scope = scope.into();
                let mut input = client.invoke(stream::iter([frame])).await?.into_inner();
                let result = terminal(&mut input).await?;
                if scope.is_empty() {
                    require(
                        matches!(result, response::Result::Failure(failure)
                        if failure.classification == v1::Classification::Refused as i32
                            && failure.code == "scope_required"),
                        "Missing Scope Was Not Refused",
                    )?;
                } else {
                    require(result == response::Result::Payload(fixture::payload(scope.as_bytes())), "Scope Did Not Reach Handler")?;
                }
            }
            let mut frame = request("conformance.step.0", fixture::payload(b"unscoped"));
            let Some(instance_invoke_request::Frame::Request(request)) = frame.frame.as_mut() else { unreachable!() };
            request.scope.clear();
            let mut input = client.invoke(stream::iter([frame])).await?.into_inner();
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"unscoped")),
                "Ordinary Unscoped Call Was Refused",
            )?;
        }
        4 => {
            for frame in [
                harmos_protocol::plugin::InstanceInvokeRequest { frame: None },
                harmos_protocol::plugin::InstanceInvokeRequest {
                    frame: Some(instance_invoke_request::Frame::Cancel(v1::Cancel { reason: "early".into() })),
                },
                harmos_protocol::plugin::InstanceInvokeRequest {
                    frame: Some(instance_invoke_request::Frame::Child(v1::ChildResponse::default())),
                },
            ] {
                invoke_status(client.invoke(stream::iter([frame])).await, Code::InvalidArgument).await?;
            }
            invoke_status(
                client.invoke(stream::iter(Vec::<harmos_protocol::plugin::InstanceInvokeRequest>::new())).await,
                Code::InvalidArgument,
            )
            .await?;
            let frames = [request("conformance.cancel", fixture::payload(b"")), request("conformance.step.0", fixture::payload(b""))];
            let mut input = client.invoke(stream::iter(frames)).await?.into_inner();
            child(&mut input, 1, "conformance.host.pending", &fixture::payload(b"")).await?;
            require(input.message().await.err().is_some_and(|status| status.code() == Code::InvalidArgument), "Repeated Request Accepted")?;
        }
        5 => {
            // Rule 5 / Override One Child Call and Preserve the Callee Policy on the Next.
            let payload = fixture::payload(b"policy");
            let (send, mut input) = invoke(&mut client, "conformance.policy", payload.clone()).await?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            let Some(instance_invoke_response::Frame::Child(overridden)) = frame else {
                return Err(Error::Invalid("Missing Policy Child".into()));
            };
            require(
                overridden.call == 1
                    && overridden.operation == "conformance.host.echo"
                    && overridden.payload == payload
                    && overridden.fingerprint == fixture::fingerprint("conformance.host.echo")
                    && overridden.policy
                        == Some(v1::OperationPolicy {
                            retries: Some(harmos_protocol::v1::RetryPolicy {
                                total_attempts: 3,
                                initial: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                                maximum: Some(prost_types::Duration { seconds: 4, nanos: 0 }),
                                deterministic_jitter: true,
                            }),
                            ..Default::default()
                        }),
                "Child Policy Override Differs",
            )?;
            send_child(&send, 1, child_response::Result::Payload(payload.clone())).await?;
            child(&mut input, 2, "conformance.host.echo", &payload).await?;
            send_child(&send, 2, child_response::Result::Payload(payload.clone())).await?;
            require(terminal(&mut input).await? == response::Result::Payload(payload), "Policy Call Changed Payload")?;
            drop(send);
            let (send, mut input) = invoke(&mut client, "conformance.children", fixture::payload(b"")).await?;
            child(&mut input, 1, "conformance.host.echo", &fixture::payload(&[1])).await?;
            child(&mut input, 2, "conformance.host.echo", &fixture::payload(&[2])).await?;
            send_child(&send, 2, child_response::Result::Payload(fixture::payload(b"second"))).await?;
            send_child(&send, 1, child_response::Result::Payload(fixture::payload(b"first"))).await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"firstsecond")),
                "Child Replies Matched by Arrival Instead of ID",
            )?;
        }
        6 => {
            let (send, mut input) = invoke(&mut client, "conformance.child_failure", fixture::payload(b"fail")).await?;
            child(&mut input, 1, "conformance.host.fail", &fixture::payload(b"fail")).await?;
            let failure = fixture::failure("conformance.host.fail");
            send_child(&send, 1, child_response::Result::Failure(failure.clone())).await?;
            require(terminal(&mut input).await? == response::Result::Failure(failure), "Child Failure Lost Fields")?;
        }
        7 => {
            let (send, mut input) = invoke(&mut client, "conformance.cancel", fixture::payload(b"pending")).await?;
            child(&mut input, 1, "conformance.host.pending", &fixture::payload(b"pending")).await?;
            send.send(harmos_protocol::plugin::InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Cancel(v1::Cancel { reason: "fixture_cancel".into() })),
            })
            .await
            .map_err(|error| Error::Invalid(error.to_string()))?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            require(frame == Some(instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call: 1 })), "Missing Child Cancel")?;
            let result = terminal(&mut input).await?;
            let response::Result::Failure(failure) = result else {
                return Err(Error::Invalid("Expected Cancelled Failure".into()));
            };
            require(
                failure.classification == v1::Classification::Cancelled as i32
                    && failure.code == "cancelled"
                    && failure.message == "fixture_cancel"
                    && failure.operation == "conformance.cancel"
                    && failure.plugin == "conformance"
                    && !failure.action.is_empty(),
                "Cancellation Evidence Differs",
            )?;
        }
        8 => {
            let (_send, mut input) = invoke(&mut client, "conformance.failure", fixture::payload(b"")).await?;
            require(
                terminal(&mut input).await? == response::Result::Failure(fixture::failure("conformance.failure")),
                "Structured Failure Lost Fields",
            )?;
        }
        9 => {
            invoke_status(client.invoke(stream::iter([request("conformance.absent", fixture::payload(b""))])).await, Code::NotFound)
                .await?;
            let mut frame = request("conformance.step.0", vec![255]);
            invoke_status(client.invoke(stream::iter([frame.clone()])).await, Code::InvalidArgument).await?;
            if let Some(instance_invoke_request::Frame::Request(request)) = &mut frame.frame {
                request.payload = fixture::payload(b"");
                request.attempt = 0;
            }
            invoke_status(client.invoke(stream::iter([frame])).await, Code::InvalidArgument).await?;
        }
        10 => {
            let (send, mut input) = open(&mut client, "conformance.ticks").await?;
            tick(&send, &mut input, 1, 1_700_000_000).await?;
            tick(&send, &mut input, 2, 1_700_000_001).await?;
            tick(&send, &mut input, 3, 1_700_003_600).await?;
            drop(send);
            require(input.message().await?.is_none(), "Samples Without Ticks After Half-Close")?;
        }
        11 => {
            let (first, mut first_input) = open(&mut client, "conformance.ticks").await?;
            let (second, mut second_input) = open(&mut client, "conformance.ticks").await?;
            let (_send, mut input) = invoke(&mut client, "conformance.step.0", fixture::payload(b"independent")).await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"independent")),
                "Idle Stream Blocked Invoke",
            )?;
            tick(&second, &mut second_input, 1, 42).await?;
            tick(&first, &mut first_input, 1, 43).await?;
        }
        12 => {
            let (send, mut input) = open(&mut client, "conformance.device").await?;
            let samples = input.message().await?.ok_or_else(|| Error::Invalid("Missing Device Sample".into()))?;
            require(
                samples.samples
                    == [v1::Sample {
                        at: None,
                        row: fixture::payload(b"device"),
                        sequence: 1,
                    }],
                "Device Sample Differs",
            )?;
            drop(send);
            require(input.message().await?.is_none(), "Device Demand Did Not End")?;
        }
        13 => {
            let (send, mut input) = open(&mut client, "conformance.ticks").await?;
            tick(&send, &mut input, 1, 100).await?;
            drop(send);
            require(input.message().await?.is_none(), "Publish Half-Close Did Not End Demand")?;
            let (send, mut input) = open(&mut client, "conformance.ticks").await?;
            tick(&send, &mut input, 1, 101).await?;
            drop(send);
            drop(input);
            let (send, input) = open(&mut client, "conformance.ticks").await?;
            drop(input);
            drop(send);
            let (_send, mut input) = invoke(&mut client, "conformance.step.0", fixture::payload(b"after close")).await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"after close")),
                "Closed Demand Broke Connection",
            )?;
        }
        14 => {
            for frame in [
                harmos_protocol::plugin::InstanceSamplesRequest { frame: None },
                harmos_protocol::plugin::InstanceSamplesRequest {
                    frame: Some(instance_samples_request::Frame::Tick(v1::Tick { at: Some(fixture::timestamp(1)) })),
                },
            ] {
                publish_status(client.samples(stream::iter([frame])).await, Code::InvalidArgument).await?;
            }
            publish_status(
                client.samples(stream::iter(Vec::<harmos_protocol::plugin::InstanceSamplesRequest>::new())).await,
                Code::InvalidArgument,
            )
            .await?;
            let open = harmos_protocol::plugin::InstanceSamplesRequest {
                frame: Some(instance_samples_request::Frame::Open(v1::SamplesSelection {
                    stream: "conformance.ticks".into(),
                    scope: "fixture".into(),
                })),
            };
            publish_status(client.samples(stream::iter([open.clone(), open.clone()])).await, Code::InvalidArgument).await?;
            // Rule 14 / Reject Missing and Out-of-Range Tick Timestamps.
            for at in [
                None,
                Some(prost_types::Timestamp { seconds: 253_402_300_800, nanos: 0 }),
                Some(prost_types::Timestamp { seconds: 0, nanos: -1 }),
                Some(prost_types::Timestamp { seconds: 0, nanos: 1_000_000_000 }),
            ] {
                let tick = harmos_protocol::plugin::InstanceSamplesRequest {
                    frame: Some(instance_samples_request::Frame::Tick(v1::Tick { at })),
                };
                publish_status(client.samples(stream::iter([open.clone(), tick])).await, Code::InvalidArgument).await?;
            }
            let missing = harmos_protocol::plugin::InstanceSamplesRequest {
                frame: Some(instance_samples_request::Frame::Open(v1::SamplesSelection {
                    stream: "conformance.absent".into(),
                    scope: "fixture".into(),
                })),
            };
            publish_status(client.samples(stream::iter([missing])).await, Code::NotFound).await?;
        }
        15 => {
            for duplicate in [false, true] {
                let (send, mut input) = invoke(&mut client, "conformance.children", fixture::payload(b"")).await?;
                child(&mut input, 1, "conformance.host.echo", &fixture::payload(&[1])).await?;
                child(&mut input, 2, "conformance.host.echo", &fixture::payload(&[2])).await?;
                if duplicate {
                    send_child(&send, 1, child_response::Result::Payload(fixture::payload(b"first"))).await?;
                }
                send_child(&send, if duplicate { 1 } else { 99 }, child_response::Result::Payload(fixture::payload(b"invalid"))).await?;
                require(
                    input.message().await.err().is_some_and(|status| status.code() == Code::InvalidArgument),
                    "Unknown or Duplicate Child Accepted",
                )?;
            }
        }
        17 => {
            // Schema Fingerprints / Check Both Roots Against the Shared General Rule.
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            let types = catalog.types.as_ref().ok_or_else(|| Error::Invalid("Missing Types".into()))?;
            let expected = fixture::catalog();
            require(catalog.operations == expected.operations, "Schema Operation Order or Contracts Differ")?;
            for operation in &expected.operations[6..] {
                let actual = catalog
                    .operations
                    .iter()
                    .find(|item| item.id == operation.id)
                    .ok_or_else(|| Error::Invalid(format!("Missing Operation: {}", operation.id)))?;
                let fingerprint = harmos_protocol::schema::fingerprint(&actual.id, types, &actual.request, &actual.response)
                    .map_err(|error| Error::Invalid(error.to_string()))?;
                require(actual == operation && actual.fingerprint == fingerprint, "General Schema Fingerprint Differs")?;
            }
            // Schema Fingerprints / Hash Request and Response Projections Independently.
            let (send, mut input) = invoke(&mut client, "conformance.projected", vec![0x12, 5, 0x0a, 3, 0x0a, 1, 255]).await?;
            drop(send);
            require(terminal(&mut input).await? == response::Result::Payload(fixture::payload(&[255])), "Projected Response Differs")?;
            // Schema Fingerprints / Retain Options, Optional Presence, Recursion and Imports.
            let actual = harmos_protocol::schema::message_types(types, "harmos.conformance.v1.Graph.Nested")
                .map_err(|error| Error::Invalid(error.to_string()))?;
            let expected = harmos_protocol::schema::message_types(&crate::conformance::types(), "harmos.conformance.v1.Graph.Nested")
                .map_err(|error| Error::Invalid(error.to_string()))?;
            require(actual == expected, "Reachable Schema Definitions Differ")?;
            // Schema Fingerprints / Round Trip a Nested Optional Zero Through the Graph.
            let graph = vec![0x0a, 2, 8, 0];
            let (send, mut input) = invoke(&mut client, "conformance.schema", graph.clone()).await?;
            drop(send);
            let mut nested = vec![8, 1, 0x12, graph.len() as u8];
            nested.extend(&graph);
            require(terminal(&mut input).await? == response::Result::Payload(nested.clone()), "Nested Graph Response Differs")?;
            let (send, mut input) = invoke(&mut client, "conformance.nested", nested).await?;
            drop(send);
            require(terminal(&mut input).await? == response::Result::Payload(graph), "Nested Root Response Differs")?;
        }
        18 => {
            // Host Stream Settings / Advertise the Setting and Its Overridden Default.
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            let expected = fixture::catalog();
            require(catalog.sections == expected.sections && catalog.streams == expected.streams, "Interval Setting or Section Differs")?;
            // Host Stream Settings / Answer Only the Host's Resolved Ticks.
            let (send, mut input) = open(&mut client, "conformance.settings").await?;
            tick(&send, &mut input, 1, 100).await?;
            tick(&send, &mut input, 2, 102).await?;
            drop(send);
            require(input.message().await?.is_none(), "Setting Observer Survived Demand Closure")?;
        }
        19 => {
            // Check Both Advertised Clock Requirements Against Fixed Fingerprints.
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            let types = catalog.types.as_ref().ok_or_else(|| Error::Invalid("Missing Types".into()))?;
            for (id, (request, response)) in fixture::CLOCK_OPERATIONS.into_iter().zip([
                ("harmos.context.v1.TimeSleepRequest", "harmos.context.v1.TimeSleepResponse"),
                ("harmos.context.v1.TimeGetRequest", "harmos.context.v1.TimeGetResponse"),
            ]) {
                let requirement = catalog
                    .requires
                    .iter()
                    .find(|item| item.id == id)
                    .ok_or_else(|| Error::Invalid(format!("Missing Clock Requirement: {id}")))?;
                let actual = harmos_protocol::schema::fingerprint(id, types, request, response)
                    .map_err(|error| Error::Invalid(error.to_string()))?;
                let host = harmos_protocol::schema::fingerprint(id, &harmos_protocol::clock::catalog_types(), request, response)
                    .map_err(|error| Error::Invalid(error.to_string()))?;
                let expected = fixture::fingerprint(id);
                require(requirement.fingerprint == expected && actual == expected && host == expected, "Clock Fingerprint Differs")?;
            }
            // Answer Host Clock Calls Without Any Local Timer.
            let (send, mut input) = invoke(&mut client, "conformance.clock", fixture::payload(&[])).await?;
            for (id, operation, payload, reply) in [
                (1, "harmos.context.v1.Time/Get", vec![], vec![0x0a, 2, 8, 100]),
                (2, "harmos.context.v1.Time/Sleep", vec![0x0a, 3, 8, 0x90, 0x1c], vec![]),
                (3, "harmos.context.v1.Time/Get", vec![], vec![0x0a, 3, 8, 0xf4, 0x1c]),
            ] {
                child(&mut input, id, operation, &payload).await?;
                send_child(&send, id, child_response::Result::Payload(reply)).await?;
            }
            drop(send);
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(&3600_i64.to_le_bytes())),
                "Clock Result Differs",
            )?;
        }
        20 => {
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            let name = "harmos.conformance.v1.PayloadOnly";
            require(
                !catalog.operations.iter().any(|item| item.request == name || item.response == name)
                    && !catalog.streams.iter().any(|item| item.row == name)
                    && !catalog.sections.iter().any(|item| item.r#type == name),
                "Payload Has An Executable Declaration",
            )?;
            let types = catalog.types.ok_or_else(|| Error::Invalid("Missing Types".into()))?;
            let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(types).map_err(|error| Error::Invalid(error.to_string()))?;
            let descriptor =
                pool.get_message_by_name(name).ok_or_else(|| Error::Invalid("Missing Independent Payload Descriptor".into()))?;
            let bytes = fixture::payload(b"record-only");
            let value =
                prost_reflect::DynamicMessage::decode(descriptor, bytes.as_slice()).map_err(|error| Error::Invalid(error.to_string()))?;
            require(value.encode_to_vec() == bytes, "Independent Payload Round Trip Differs")?;
        }
        21 => {
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            for (id, visibility, replay_safe) in [
                ("example.plugin.v1.Lifecycle/Initialize", v1::Visibility::Protected, false),
                ("example.plugin.v1.Steps/Execute", v1::Visibility::Public, true),
                ("conformance.step.0", v1::Visibility::Public, false),
            ] {
                let operation = catalog
                    .operations
                    .iter()
                    .find(|operation| operation.id == id)
                    .ok_or_else(|| Error::Invalid(format!("Missing Operation: {id}")))?;
                require(
                    operation.visibility == visibility as i32 && operation.replay_safe == replay_safe,
                    "Visibility or Replay Safety Differs",
                )?;
            }
        }
        22 => {
            // Interleave Calls Across Two Independent Invocation Resource Stores.
            let first_payload = fixture::payload(b"first");
            let second_payload = fixture::payload(b"second");
            let (first_send, mut first) = invoke(&mut client, "example.plugin.v1.Steps/Execute", first_payload.clone()).await?;
            child(&mut first, 1, "conformance.host.echo", &first_payload).await?;
            let (second_send, mut second) = invoke(&mut client, "example.plugin.v1.Steps/Execute", second_payload.clone()).await?;
            child(&mut second, 1, "conformance.host.echo", &second_payload).await?;
            send_child(&second_send, 1, child_response::Result::Payload(second_payload.clone())).await?;
            require(terminal(&mut second).await? == response::Result::Payload(second_payload), "Second Invocation Resource Differs")?;
            send_child(&first_send, 1, child_response::Result::Payload(first_payload.clone())).await?;
            require(terminal(&mut first).await? == response::Result::Payload(first_payload), "First Invocation Resource Was Replaced")?;
        }
        23 => {
            let payload = fixture::payload(b"ready");
            let (send, mut input) = invoke(&mut client, "conformance.cancel_reason", payload.clone()).await?;
            child(&mut input, 1, "conformance.host.echo", &payload).await?;
            send_child(&send, 1, child_response::Result::Payload(payload)).await?;
            send.send(harmos_protocol::plugin::InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Cancel(v1::Cancel { reason: "Requested Stop".into() })),
            })
            .await
            .map_err(|_| Error::Invalid("Cancel Closed".into()))?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"Requested Stop")),
                "Cancellation Reason Did Not Reach Handler",
            )?;
        }
        24 => {
            let description =
                client.get(harmos_protocol::plugin::InstanceGetRequest { revision: 1, ..Default::default() }).await?.into_inner();
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Catalog Missing".into()))?;
            require(
                !catalog.operations.iter().any(|operation| operation.id == "conformance.local"),
                "Private Operation Leaked Into Catalog",
            )?;
            require(
                !catalog.requires.iter().any(|operation| operation.id == "conformance.local"),
                "Private Requirement Leaked Into Catalog",
            )?;
            let payload = fixture::payload(b"local payload");
            let (_send, mut input) = invoke(&mut client, "conformance.local_call", payload.clone()).await?;
            // terminal Rejects Any Child Frame, So Private Dispatch Cannot Use the Carrier.
            require(terminal(&mut input).await? == response::Result::Payload(payload), "Local Private Result Differs")?;
            let result = invoke(&mut client, "conformance.local", fixture::payload(b"outside")).await;
            match result {
                Err(Error::Status(status)) if status.code() == Code::NotFound => {}
                Ok((_send, mut input)) => {
                    require(
                        input.message().await.is_err_and(|status| status.code() == Code::NotFound),
                        "Private Carrier Call Was Admitted",
                    )?;
                }
                _ => return Err(Error::Invalid("Private Carrier Call Was Admitted".into())),
            }
        }
        29 => {
            for (mode, operation) in [
                (b"public".as_slice(), "conformance.peer.public"),
                (b"self", "example.plugin.v1.Lifecycle/Initialize"),
                (b"protected", "conformance.peer.protected"),
                (b"local", "conformance.peer.local"),
            ] {
                let payload = fixture::payload(mode);
                let (send, mut input) = invoke(&mut client, "conformance.peer_call", payload.clone()).await?;
                child(&mut input, 1, operation, &if mode == b"public" { fixture::payload(b"fixture") } else { payload.clone() }).await?;
                if mode == b"self" {
                    send_child(&send, 1, child_response::Result::Payload(payload.clone())).await?;
                    require(
                        terminal(&mut input).await? == response::Result::Payload(payload),
                        "Declaring Plugin Cannot Call Its Protected Operation",
                    )?;
                } else if mode == b"public" {
                    let payload = fixture::payload(b"peer result");
                    send_child(&send, 1, child_response::Result::Payload(payload.clone())).await?;
                    require(
                        terminal(&mut input).await? == response::Result::Payload(payload),
                        "Public Peer Result Did Not Cross the Host",
                    )?;
                } else {
                    let mut failure = fixture::failure(operation);
                    failure.classification = v1::Classification::Refused as i32;
                    failure.message = format!(
                        "Invoke {operation}: {}",
                        if mode == b"protected" { "Protected Peer Operation" } else { "Operation Is Not Declared in the Peer Catalog" }
                    );
                    send_child(&send, 1, child_response::Result::Failure(failure.clone())).await?;
                    require(terminal(&mut input).await? == response::Result::Failure(failure), "Peer Refusal Lost Its Action or Cause")?;
                }
            }
        }
        27 => {
            for mode in [b"deadline".as_slice(), b"overall"] {
                let (send, mut input) = invoke(&mut client, "conformance.local_call", fixture::payload(mode)).await?;
                for call in 1..=2 {
                    child(&mut input, call, "harmos.context.v1.Time/Get", &[]).await?;
                    send_child(
                        &send,
                        call,
                        child_response::Result::Payload(
                            harmos_protocol::context::TimeGetResponse {
                                now: Some(prost_types::Timestamp { seconds: 0, nanos: 0 }),
                            }
                            .encode_to_vec(),
                        ),
                    )
                    .await?;
                }
                let mut deadline = None;
                let mut sleeping = None;
                for _ in 0..2 {
                    let frame = input.message().await?.and_then(|frame| frame.frame);
                    let Some(instance_invoke_response::Frame::Child(child)) = frame else {
                        return Err(Error::Invalid("Private Deadline: Expected Clock Child".into()));
                    };
                    require(child.operation == "harmos.context.v1.Time/Sleep", "Private Operation Leaked Onto Carrier")?;
                    let duration = harmos_protocol::context::TimeSleepRequest::decode(child.payload.as_slice())?.duration.unwrap();
                    if child.background {
                        require(duration == prost_types::Duration { seconds: 2, nanos: 0 }, "Private Attempt Budget Differs")?;
                        deadline = Some(child.call);
                    } else {
                        require(duration == prost_types::Duration { seconds: 3600, nanos: 0 }, "Private Handler Sleep Differs")?;
                        sleeping = Some(child.call);
                    }
                }
                send_child(
                    &send,
                    deadline.ok_or_else(|| Error::Invalid("Private Deadline: Guard Missing".into()))?,
                    child_response::Result::Payload(Vec::new()),
                )
                .await?;
                let mut cancelled = false;
                loop {
                    let frame = input.message().await?.and_then(|frame| frame.frame);
                    match frame {
                        Some(instance_invoke_response::Frame::ChildCancel(child)) => {
                            if Some(child.call) == sleeping {
                                cancelled = true;
                            }
                        }
                        Some(instance_invoke_response::Frame::Response(v1::Response {
                            result: Some(response::Result::Failure(failure)),
                        })) => {
                            require(
                                cancelled && failure.classification() == v1::Classification::TimedOut,
                                "Private Deadline Did Not Cancel Its Child Before Settling",
                            )?;
                            require(input.message().await?.is_none(), "Frame After Private Deadline")?;
                            break;
                        }
                        _ => return Err(Error::Invalid(format!("Private Deadline: Unexpected Frame {frame:?}"))),
                    }
                }
            }
        }
        28 => {
            let payload = fixture::payload(b"cancel");
            let (send, mut input) = invoke(&mut client, "conformance.local_call", payload.clone()).await?;
            child(&mut input, 1, "conformance.host.echo", &payload).await?;
            send_child(&send, 1, child_response::Result::Payload(payload)).await?;
            send.send(harmos_protocol::plugin::InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Cancel(v1::Cancel { reason: "Local Stop".into() })),
            })
            .await
            .map_err(|_| Error::Invalid("Private Cancel: Carrier Closed".into()))?;
            let mut guards = std::collections::BTreeSet::new();
            loop {
                let frame = input.message().await?.and_then(|frame| frame.frame);
                match frame {
                    Some(instance_invoke_response::Frame::Child(child)) => {
                        require(
                            child.background && child.operation == "harmos.context.v1.Time/Sleep",
                            "Private Cancellation Leaked a Local Call",
                        )?;
                        guards.insert(child.call);
                    }
                    Some(instance_invoke_response::Frame::ChildCancel(child)) => {
                        guards.remove(&child.call);
                    }
                    Some(instance_invoke_response::Frame::Response(v1::Response {
                        result: Some(response::Result::Payload(payload)),
                    })) => {
                        require(
                            guards.is_empty() && payload == fixture::payload(b"Local Stop"),
                            "Private Cancellation Did Not Preserve Its Reason and Settle Guards",
                        )?;
                        require(input.message().await?.is_none(), "Frame After Private Cancellation")?;
                        break;
                    }
                    _ => return Err(Error::Invalid(format!("Private Cancel: Unexpected Frame {frame:?}"))),
                }
            }
        }
        26 => {
            let (send, mut input) = invoke(&mut client, "conformance.local_call", fixture::payload(b"retry")).await?;
            let delay = harmos_protocol::context::TimeSleepRequest {
                duration: Some(prost_types::Duration { seconds: 1, nanos: 11 }),
            }
            .encode_to_vec();
            child(&mut input, 1, "harmos.context.v1.Time/Sleep", &delay).await?;
            send_child(&send, 1, child_response::Result::Payload(Vec::new())).await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"2")),
                "Private Retry Did Not Preserve Attempt Semantics",
            )?;
            // An Hour-Long Retry Must Stop at the Remaining Overall Budget.
            let (send, mut input) = invoke(&mut client, "conformance.local_call", fixture::payload(b"retry-bound")).await?;
            let mut now = 0;
            let mut slept = false;
            let mut guards = std::collections::BTreeSet::new();
            loop {
                let frame = input.message().await?.and_then(|frame| frame.frame);
                match frame {
                    Some(instance_invoke_response::Frame::Child(child)) => {
                        require(child.fingerprint == fixture::fingerprint(&child.operation), "Private Retry: Clock Fingerprint Differs")?;
                        if child.operation == "harmos.context.v1.Time/Get" {
                            send_child(
                                &send,
                                child.call,
                                child_response::Result::Payload(
                                    harmos_protocol::context::TimeGetResponse {
                                        now: Some(prost_types::Timestamp { seconds: now, nanos: 0 }),
                                    }
                                    .encode_to_vec(),
                                ),
                            )
                            .await?;
                        } else {
                            require(child.operation == "harmos.context.v1.Time/Sleep", "Private Retry Leaked Onto the Carrier")?;
                            let duration = harmos_protocol::context::TimeSleepRequest::decode(child.payload.as_slice())?.duration.unwrap();
                            require(duration == prost_types::Duration { seconds: 1, nanos: 0 }, "Retry Outlived the Overall Deadline")?;
                            if child.background {
                                guards.insert(child.call);
                            } else {
                                require(!slept, "Private Retry Continued After Its Overall Deadline")?;
                                slept = true;
                                now = 1;
                                send_child(&send, child.call, child_response::Result::Payload(Vec::new())).await?;
                            }
                        }
                    }
                    Some(instance_invoke_response::Frame::ChildCancel(cancel)) => {
                        guards.remove(&cancel.call);
                    }
                    Some(instance_invoke_response::Frame::Response(v1::Response {
                        result: Some(response::Result::Failure(failure)),
                    })) => {
                        require(
                            slept && guards.is_empty() && failure.classification() == v1::Classification::TimedOut,
                            &format!(
                                "Private Retry Did Not Settle at Its Overall Deadline: slept={slept}, guards={}, failure={failure:?}",
                                guards.len()
                            ),
                        )?;
                        require(input.message().await?.is_none(), "Frame After Private Retry Deadline")?;
                        break;
                    }
                    _ => return Err(Error::Invalid(format!("Private Retry: Unexpected Frame {frame:?}"))),
                }
            }
        }
        25 => {
            for mode in [b"normal".as_slice(), b"cancel", b"failure"] {
                let (send, mut input) = invoke(&mut client, "conformance.watch", fixture::payload(mode)).await?;
                let frame = input.message().await?.and_then(|frame| frame.frame);
                let Some(instance_invoke_response::Frame::Child(child)) = frame else {
                    return Err(Error::Invalid("Watch: Missing Open".into()));
                };
                require(
                    child.operation == "conformance.host.watch" && child.watch && child.max_response_bytes == 4096,
                    "Watch Open Differs",
                )?;
                let call = child.call;
                for bytes in [b"first".as_slice(), b"second"] {
                    let frame = input.message().await?.and_then(|frame| frame.frame);
                    require(
                        frame == Some(instance_invoke_response::Frame::ChildDemand(v1::ChildDemand { call })),
                        "Watch Item Without Demand",
                    )?;
                    send_child(&send, call, child_response::Result::Item(fixture::payload(bytes))).await?;
                    if mode == b"cancel" {
                        break;
                    }
                }
                if mode == b"cancel" {
                    require(
                        input.message().await?.and_then(|frame| frame.frame)
                            == Some(instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call })),
                        "Watch Demand Not Cancelled",
                    )?;
                    require(
                        terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"first")),
                        "Watch Cancel Result Differs",
                    )?;
                } else {
                    require(
                        input.message().await?.and_then(|frame| frame.frame)
                            == Some(instance_invoke_response::Frame::ChildDemand(v1::ChildDemand { call })),
                        "Watch Terminal Demand Missing",
                    )?;
                    if mode == b"failure" {
                        let failure = fixture::failure("conformance.host.watch");
                        send_child(&send, call, child_response::Result::Failure(failure.clone())).await?;
                        require(terminal(&mut input).await? == response::Result::Failure(failure), "Watch Failure Cause Changed")?;
                    } else {
                        send_child(&send, call, child_response::Result::Payload(Vec::new())).await?;
                        require(
                            terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"firstsecond")),
                            "Watch Result Differs",
                        )?;
                    }
                }
            }
        }
        16 => {
            client.stop(harmos_protocol::plugin::InstanceStopRequest { reason: "Conformance Complete".into() }).await?;
            if !managed {
                connect(address).await?;
            }
        }
        _ => return Err(Error::Invalid("Unknown Rule".into())),
    }
    Ok(())
}

/* === Tests === */
