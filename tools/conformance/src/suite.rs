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
use prost_types::{Duration as ProtoDuration, Timestamp};
use std::{
    collections::{BTreeSet, HashMap},
    result::Result as StdResult,
};
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
use harmos_protocol::plugin::{child_response, response};
use harmos_protocol::plugin::{
    instance_client::InstanceClient, instance_invoke_request, instance_invoke_response, instance_samples_request, instance_samples_response,
};

use harmos_protocol::host::plugins::SettingsListResponse;
use harmos_protocol::host::{OperationsGetRequest, OperationsGetResponse, OperationsListResponse, TimeGetResponse, TimeSleepRequest};
use harmos_protocol::model::{
    Classification, DiscoveredContract, DiscoveredOperation, Failure, OperationPolicy, SettingValues, SettingsDeclarations, Visibility,
};
use harmos_protocol::model::{RetryPolicy, StorageTransferSettings, TransferFraming};
use harmos_protocol::plugin::{
    Cancel, ChildCancel, ChildDemand, ChildRequest, ChildResponse, Request, Response, Sample, SamplesSelection, Tick,
};
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
    InstanceSamplesResponse, InstanceStopRequest,
};

/* === Constants === */

pub const CASES: &[&str] = &[
    "Get and Declarations",
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
    "Samples Child Calls and Correlation",
    "Samples Child Routing and Failures",
    "Samples Child Lifetime and Refusal",
    "Client Caller Refusal",
    "Plugin Caller Refusal",
    "Wire Operation Identities",
    "Message Ceiling Independent of Transfer Chunks",
    "Typed Settings Defaults and Stored Values",
    "Typed Operations Discovery",
];

static INVOCATION: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

type Client = InstanceClient<Channel>;
type Invoke = (mpsc::Sender<InstanceInvokeRequest>, Streaming<InstanceInvokeResponse>);
type Publish = (mpsc::Sender<InstanceSamplesRequest>, Streaming<InstanceSamplesResponse>);

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
        let result = tokio::time::timeout(Duration::from_secs(30), Box::pin(case(rule, address, managed))).await;
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

async fn connect(address: &str) -> Result<(Client, InstanceGetResponse)> {
    let maximum = harmos_protocol::transfer::default_message_bytes().get();
    let mut client = Client::connect(address.to_owned()).await?.max_decoding_message_size(maximum).max_encoding_message_size(maximum);
    let description = describe(&mut client).await?;
    Ok((client, description))
}

async fn describe(client: &mut Client) -> Result<InstanceGetResponse> {
    Ok(client
        .get(InstanceGetRequest {
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

fn request(operation: &str, payload: Vec<u8>) -> InstanceInvokeRequest {
    let invocation = INVOCATION.fetch_add(1, Ordering::Relaxed);
    InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Request(Request {
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

async fn terminal(input: &mut Streaming<InstanceInvokeResponse>) -> Result<response::Result> {
    let frame = input.message().await?.and_then(|frame| frame.frame);
    let Some(instance_invoke_response::Frame::Response(Response { result: Some(result) })) = frame else {
        return Err(Error::Invalid(format!("Expected Terminal Response, Got {frame:?}")));
    };
    require(input.message().await?.is_none(), "Frames After Terminal Response")?;
    Ok(result)
}

async fn child(input: &mut Streaming<InstanceInvokeResponse>, id: u64, operation: &str, payload: &[u8]) -> Result<()> {
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

async fn send_child(send: &mpsc::Sender<InstanceInvokeRequest>, call: u64, result: child_response::Result) -> Result<()> {
    send.send(InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Child(ChildResponse { call, result: Some(result) })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))
}

async fn open(client: &mut Client, name: &str) -> Result<Publish> {
    let (send, receive) = mpsc::channel(8);
    send.send(InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Open(SamplesSelection {
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
    send: &mpsc::Sender<InstanceSamplesRequest>, input: &mut Streaming<InstanceSamplesResponse>, sequence: u64, seconds: i64,
) -> Result<()> {
    let at = fixture::timestamp(seconds);
    send.send(InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Tick(Tick { at: Some(at) })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))?;
    let samples = input.message().await?.ok_or_else(|| Error::Invalid("Missing Tick Sample".into()))?;
    let expected = Sample {
        at: Some(at),
        row: fixture::payload(&sequence.to_le_bytes()),
        sequence,
    };
    require(samples.samples == [expected], "Tick Time, Row, Sequence or Sample Count Differs")
}

async fn samples_tick(send: &mpsc::Sender<InstanceSamplesRequest>, seconds: i64) -> Result<()> {
    send.send(InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Tick(Tick { at: Some(fixture::timestamp(seconds)) })),
    })
    .await
    .map_err(|error| Error::Invalid(format!("Send Tick: {error}")))
}

async fn samples_child(input: &mut Streaming<InstanceSamplesResponse>, operation: &str, payload: &[u8]) -> Result<ChildRequest> {
    let frame = input.message().await?.ok_or_else(|| Error::Invalid("Read Samples Child: Missing Frame".into()))?;
    require(frame.samples.is_empty(), "Read Samples Child: Child Frame Also Answers Tick")?;
    let Some(instance_samples_response::Frame::Child(child)) = frame.frame else {
        return Err(Error::Invalid("Read Samples Child: Expected Request".into()));
    };
    require(
        child.operation == operation && child.payload == payload && child.fingerprint == fixture::fingerprint(operation),
        "Read Samples Child: Contract Differs",
    )?;
    Ok(child)
}

async fn samples_reply(send: &mpsc::Sender<InstanceSamplesRequest>, call: u64, result: child_response::Result) -> Result<()> {
    send.send(InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Child(ChildResponse { call, result: Some(result) })),
    })
    .await
    .map_err(|error| Error::Invalid(format!("Reply to Samples Child: {error}")))
}

async fn samples_answer(input: &mut Streaming<InstanceSamplesResponse>, sequence: u64, seconds: i64) -> Result<Vec<u8>> {
    let frame = input.message().await?.ok_or_else(|| Error::Invalid("Read Tick Answer: Missing Frame".into()))?;
    require(frame.frame.is_none() && frame.samples.len() == 1, "Read Tick Answer: Expected One Sample Without Child Traffic")?;
    let sample = &frame.samples[0];
    require(sample.sequence == sequence && sample.at == Some(fixture::timestamp(seconds)), "Read Tick Answer: Sequence or Clock Differs")?;
    Ok(fixture::Value::decode(sample.row.as_slice())?.value)
}

async fn samples_control(input: &mut Streaming<InstanceSamplesResponse>, expected: instance_samples_response::Frame) -> Result<()> {
    let frame = input.message().await?.ok_or_else(|| Error::Invalid("Read Samples Control: Missing Frame".into()))?;
    require(frame.samples.is_empty() && frame.frame == Some(expected), "Read Samples Control: Demand or Cancellation Differs")
}

async fn invoke_status(result: StdResult<tonic::Response<Streaming<InstanceInvokeResponse>>, tonic::Status>, expected: Code) -> Result<()> {
    let status = match result {
        Err(status) => status,
        Ok(response) => response.into_inner().message().await.err().ok_or_else(|| Error::Invalid("Expected gRPC Error".into()))?,
    };
    require(status.code() == expected, &format!("Expected {expected:?}, Got {status}"))
}

async fn publish_status(
    result: StdResult<tonic::Response<Streaming<InstanceSamplesResponse>>, tonic::Status>, expected: Code,
) -> Result<()> {
    let status = match result {
        Err(status) => status,
        Ok(response) => response.into_inner().message().await.err().ok_or_else(|| Error::Invalid("Expected gRPC Error".into()))?,
    };
    require(status.code() == expected, &format!("Expected {expected:?}, Got {status}"))
}

async fn samples_case(rule: usize, client: &mut Client) -> Result<()> {
    match rule {
        30 => {
            let (send, mut input) = open(client, "conformance.tick_children").await?;
            let mut used = BTreeSet::new();
            for (sequence, seconds) in [(1, 100), (2, 101)] {
                samples_tick(&send, seconds).await?;
                let first = samples_child(&mut input, "conformance.host.echo", &fixture::payload(&[1])).await?;
                let second = samples_child(&mut input, "conformance.host.echo", &fixture::payload(&[2])).await?;
                require(used.insert(first.call) && used.insert(second.call), "Samples: Child ID Reused Across Ticks")?;
                // Interleave Another Samples Call and Invoke Before Resolving the First Tick.
                let (other_send, mut other) = open(client, "conformance.tick_children").await?;
                samples_tick(&other_send, 200).await?;
                let other_first = samples_child(&mut other, "conformance.host.echo", &fixture::payload(&[1])).await?;
                let other_second = samples_child(&mut other, "conformance.host.echo", &fixture::payload(&[2])).await?;
                let payload = fixture::payload(b"independent");
                let (_invoke_send, mut invocation) = invoke(client, "conformance.step.0", payload.clone()).await?;
                require(terminal(&mut invocation).await? == response::Result::Payload(payload), "Samples: Tick Blocked Invoke")?;
                samples_reply(&other_send, other_second.call, child_response::Result::Payload(fixture::payload(&[8]))).await?;
                samples_reply(&other_send, other_first.call, child_response::Result::Payload(fixture::payload(&[7]))).await?;
                require(samples_answer(&mut other, 1, 200).await? == [7, 8], "Samples: Separate Call Correlation Differs")?;
                samples_reply(&send, second.call, child_response::Result::Payload(fixture::payload(&[2]))).await?;
                samples_reply(&send, first.call, child_response::Result::Payload(fixture::payload(&[1]))).await?;
                require(samples_answer(&mut input, sequence, seconds).await? == [1, 2], "Samples: Reverse Reply Correlation Differs")?;
            }
            samples_tick(&send, 102).await?;
            let _first = samples_child(&mut input, "conformance.host.echo", &fixture::payload(&[1])).await?;
            let _second = samples_child(&mut input, "conformance.host.echo", &fixture::payload(&[2])).await?;
            samples_reply(&send, *used.first().expect("earlier tick child"), child_response::Result::Payload(fixture::payload(&[9])))
                .await?;
            require(
                input.message().await.is_err_and(|status| status.code() == Code::InvalidArgument),
                "Samples: Earlier Tick Reply Assigned to Later Tick",
            )?;
            for duplicate in [false, true] {
                let (send, mut input) = open(client, "conformance.tick_children").await?;
                samples_tick(&send, 300).await?;
                let first = samples_child(&mut input, "conformance.host.echo", &fixture::payload(&[1])).await?;
                let _second = samples_child(&mut input, "conformance.host.echo", &fixture::payload(&[2])).await?;
                if duplicate {
                    samples_reply(&send, first.call, child_response::Result::Payload(fixture::payload(&[1]))).await?;
                }
                samples_reply(
                    &send,
                    if duplicate { first.call } else { u64::MAX },
                    child_response::Result::Payload(fixture::payload(&[9])),
                )
                .await?;
                require(
                    input.message().await.is_err_and(|status| status.code() == Code::InvalidArgument),
                    "Samples: Unknown or Duplicate Reply Accepted",
                )?;
            }
        }
        31 => {
            let (send, mut input) = open(client, "conformance.tick_context").await?;
            samples_tick(&send, 400).await?;
            let echo = samples_child(&mut input, "conformance.host.echo", &fixture::payload(b"fixture")).await?;
            let echo_reply = fixture::payload(b"scoped");
            samples_reply(&send, echo.call, child_response::Result::Payload(echo_reply.clone())).await?;
            let clock = samples_child(&mut input, "harmos.host.v1.Time/Get", &[]).await?;
            let clock_reply = TimeGetResponse { now: Some(fixture::timestamp(400)) }.encode_to_vec();
            samples_reply(&send, clock.call, child_response::Result::Payload(clock_reply.clone())).await?;
            let settings = samples_child(&mut input, "harmos.host.v1.plugins.Settings/List", &[]).await?;
            let settings_reply = SettingsListResponse {
                value: Some(fixture::settings_values(true)),
                revision: vec![2],
                resolved_at: None,
                declarations: fixture::settings_declarations().values,
            }
            .encode_to_vec();
            samples_reply(&send, settings.call, child_response::Result::Payload(settings_reply.clone())).await?;
            let answer = samples_answer(&mut input, 1, 400).await?;
            let mut remaining = answer.as_slice();
            let mut replies = Vec::new();
            for _ in 0..3 {
                let length: [u8; 4] = remaining
                    .get(..4)
                    .ok_or_else(|| Error::Invalid("Samples: Missing Context Result Length".into()))?
                    .try_into()
                    .expect("four bytes");
                let length = u32::from_le_bytes(length) as usize;
                remaining = &remaining[4..];
                let payload = remaining.get(..length).ok_or_else(|| Error::Invalid("Samples: Truncated Context Result".into()))?;
                replies.push(payload);
                remaining = &remaining[length..];
            }
            require(remaining.is_empty() && replies[0] == echo_reply, "Samples: Scoped Echo Differs")?;
            require(
                TimeGetResponse::decode(replies[1])? == TimeGetResponse::decode(clock_reply.as_slice())?,
                "Samples: Clock Result Differs",
            )?;
            require(
                SettingsListResponse::decode(replies[2])? == SettingsListResponse::decode(settings_reply.as_slice())?,
                "Samples: Typed Settings Result Differs",
            )?;

            let (send, mut input) = open(client, "conformance.tick_failure").await?;
            samples_tick(&send, 401).await?;
            let child = samples_child(&mut input, "conformance.host.fail", &fixture::payload(&[])).await?;
            let failure = fixture::failure("conformance.host.fail");
            samples_reply(&send, child.call, child_response::Result::Failure(failure.clone())).await?;
            let answer = samples_answer(&mut input, 1, 401).await?;
            require(Failure::decode(answer.as_slice())? == failure, "Samples: Structured Failure Changed")?;

            let (send, mut input) = open(client, "conformance.tick_watch").await?;
            samples_tick(&send, 402).await?;
            let watch = samples_child(&mut input, "conformance.host.watch", &fixture::payload(b"fixture")).await?;
            require(watch.watch && watch.max_response_bytes == 4096, "Samples: Watch Bounds Differ")?;
            for item in [7, 8] {
                samples_control(&mut input, instance_samples_response::Frame::ChildDemand(ChildDemand { call: watch.call })).await?;
                samples_reply(&send, watch.call, child_response::Result::Item(fixture::payload(&[item]))).await?;
            }
            samples_control(&mut input, instance_samples_response::Frame::ChildDemand(ChildDemand { call: watch.call })).await?;
            samples_reply(&send, watch.call, child_response::Result::Payload(fixture::payload(&[]))).await?;
            require(samples_answer(&mut input, 1, 402).await? == [7, 8], "Samples: Watch Items Differ")?;
        }
        32 => {
            for name in ["conformance.ticks", "conformance.device"] {
                let (send, mut input) = open(client, name).await?;
                if name == "conformance.device" {
                    require(input.message().await?.is_some(), "Samples: Missing Device Row")?;
                }
                samples_reply(&send, 1, child_response::Result::Payload(Vec::new())).await?;
                require(
                    input.message().await.is_err_and(|status| status.code() == Code::InvalidArgument),
                    "Samples: Child Outside Tick Accepted",
                )?;
            }
            let (send, mut input) = open(client, "conformance.tick_cancel").await?;
            samples_tick(&send, 500).await?;
            let pending = samples_child(&mut input, "conformance.host.pending", &fixture::payload(&[])).await?;
            let echo = samples_child(&mut input, "conformance.host.echo", &fixture::payload(b"release")).await?;
            samples_reply(&send, echo.call, child_response::Result::Payload(fixture::payload(b"released"))).await?;
            samples_control(&mut input, instance_samples_response::Frame::ChildCancel(ChildCancel { call: pending.call })).await?;
            let automatic = samples_child(&mut input, "conformance.host.pending", &fixture::payload(&[])).await?;
            samples_control(&mut input, instance_samples_response::Frame::ChildCancel(ChildCancel { call: automatic.call })).await?;
            require(samples_answer(&mut input, 1, 500).await? == b"cancelled", "Samples: Cancellation Answer Differs")?;
            samples_reply(&send, pending.call, child_response::Result::Payload(Vec::new())).await?;
            require(
                input.message().await.is_err_and(|status| status.code() == Code::InvalidArgument),
                "Samples: Late Child After Tick Accepted",
            )?;
            let (send, mut input) = open(client, "conformance.tick_cancel").await?;
            samples_tick(&send, 501).await?;
            let _pending = samples_child(&mut input, "conformance.host.pending", &fixture::payload(&[])).await?;
            let _echo = samples_child(&mut input, "conformance.host.echo", &fixture::payload(b"release")).await?;
            drop(send);
            while let Some(frame) = input.message().await? {
                require(
                    frame.samples.is_empty() && matches!(frame.frame, Some(instance_samples_response::Frame::ChildCancel(_))),
                    "Samples: Row or New Child After Demand End",
                )?;
            }
            let (send, mut input) = open(client, "conformance.tick_cancel").await?;
            samples_tick(&send, 501).await?;
            let _pending = samples_child(&mut input, "conformance.host.pending", &fixture::payload(&[])).await?;
            let _echo = samples_child(&mut input, "conformance.host.echo", &fixture::payload(b"release")).await?;
            drop(input);
            drop(send);
            let (send, mut input) = open(client, "conformance.ticks").await?;
            tick(&send, &mut input, 1, 502).await?;
            let payload = fixture::payload(b"after samples cancellation");
            let (_send, mut input) = invoke(client, "conformance.step.0", payload.clone()).await?;
            require(terminal(&mut input).await? == response::Result::Payload(payload), "Samples: Cancellation Broke Invoke")?;
        }
        _ => return Err(Error::Invalid("Run Samples Case: Unknown Rule".into())),
    }
    Ok(())
}

async fn case(rule: usize, address: &str, managed: bool) -> Result<()> {
    let (mut client, description) = connect(address).await?;
    match rule {
        33 | 34 => crate::callers::run(rule, address).await?,
        30..=32 => Box::pin(samples_case(rule, &mut client)).await?,
        37 => {
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("List Settings: Missing Declarations".into()))?;
            let section = declarations
                .sections
                .iter()
                .find(|section| section.r#type == "harmos.model.v1.SettingsDeclarations")
                .ok_or_else(|| Error::Invalid("List Settings: Missing Declarations".into()))?;
            let declarations = SettingsDeclarations::decode(section.payload.as_slice())?;
            require(declarations.values.len() == fixture::settings_values(false).values.len(), "List Settings: Declaration Count Differs")?;
            let declared: HashMap<_, _> = declarations.values.into_iter().map(|value| (value.name, value.default)).collect();
            let expected: HashMap<_, _> =
                fixture::settings_values(false).values.into_iter().map(|(name, value)| (name, Some(value))).collect();
            require(declared == expected, "List Settings: Typed Defaults Differ")?;
            for stored in [false, true] {
                let (send, mut input) = invoke(&mut client, "conformance.settings_read", fixture::payload(&[])).await?;
                child(&mut input, 1, "harmos.host.v1.plugins.Settings/List", &[]).await?;
                let values = fixture::settings_values(stored);
                let reply = SettingsListResponse {
                    value: Some(values.clone()),
                    revision: vec![if stored { 2 } else { 1 }],
                    resolved_at: None,
                    declarations: fixture::settings_declarations().values,
                };
                send_child(&send, 1, child_response::Result::Payload(reply.encode_to_vec())).await?;
                let response::Result::Payload(payload) = terminal(&mut input).await? else {
                    return Err(Error::Invalid("List Settings: Expected Values".into()));
                };
                let value = fixture::Value::decode(payload.as_slice())?;
                require(SettingValues::decode(value.value.as_slice())? == values, "List Settings: Values Differ")?;
            }
        }
        38 => {
            let (send, mut input) = invoke(&mut client, "conformance.discover", fixture::payload(&[])).await?;
            child(&mut input, 1, "harmos.host.v1.Operations/List", &[]).await?;
            let operation = DiscoveredOperation {
                owner: "host".into(),
                generation: 7,
                contract: Some(DiscoveredContract {
                    id: "conformance.host.echo".into(),
                    fingerprint: fixture::fingerprint("conformance.host.echo"),
                    visibility: Visibility::Public as i32,
                    ..Default::default()
                }),
                grpc_available: true,
                ..Default::default()
            };
            send_child(
                &send,
                1,
                child_response::Result::Payload(OperationsListResponse { operations: vec![operation.clone()] }.encode_to_vec()),
            )
            .await?;
            let request = OperationsGetRequest { id: "conformance.host.echo".into() };
            child(&mut input, 2, "harmos.host.v1.Operations/Get", &request.encode_to_vec()).await?;
            send_child(&send, 2, child_response::Result::Payload(OperationsGetResponse { operation: Some(operation) }.encode_to_vec()))
                .await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"conformance.host.echo")),
                "Discover Operation: Typed Contract Differs",
            )?;
        }
        36 => {
            // Echo a Message Larger Than the Storage Chunk Through the Default Carrier.
            let chunk = StorageTransferSettings::default().chunk_bytes() as usize;
            let payload = fixture::payload(&vec![0x5a; chunk + TransferFraming::Bytes as usize]);
            let (_send, mut input) = invoke(&mut client, "conformance.step.0", payload.clone()).await?;
            require(terminal(&mut input).await? == response::Result::Payload(payload), "Invoke Large Echo: Payload Differs")?;
        }
        35 => {
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Declare: Missing Declarations".into()))?;
            for operation in &declarations.operations {
                harmos_protocol::schema::plugin_operation_identity(&operation.id).map_err(|error| Error::Invalid(error.to_string()))?;
            }
            for (id, visibility) in [
                ("conformance.step.0", Visibility::Public),
                ("example.plugin.v1.Lifecycle/Initialize", Visibility::Protected),
                ("example.plugin.v1.Steps/Execute", Visibility::Public),
            ] {
                let operation = declarations
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
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Missing Declarations".into()))?;
            let expected = fixture::declarations();
            require(
                expected.operations.iter().all(|entry| declarations.operations.iter().find(|actual| actual.id == entry.id) == Some(entry))
                    && declarations.requires.len() == expected.requires.len()
                    && expected
                        .requires
                        .iter()
                        .all(|entry| declarations.requires.iter().find(|actual| actual.id == entry.id) == Some(entry))
                    && declarations.streams.get(..2) == expected.streams.get(..2),
                "Fixed Fixture Declarations Differs",
            )?;
            require(declarations.digest == harmos_protocol::schema::declarations_digest(&declarations), "Declarations Digest Differs")?;
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
            require(
                second.started_at == Some(at) && second.declarations == first.declarations,
                "Reconnect Changed Process Identity or Declarations",
            )?;
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
                        if failure.classification == Classification::Refused as i32
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
                InstanceInvokeRequest { frame: None },
                InstanceInvokeRequest {
                    frame: Some(instance_invoke_request::Frame::Cancel(Cancel { reason: "early".into() })),
                },
                InstanceInvokeRequest {
                    frame: Some(instance_invoke_request::Frame::Child(ChildResponse::default())),
                },
            ] {
                invoke_status(client.invoke(stream::iter([frame])).await, Code::InvalidArgument).await?;
            }
            invoke_status(client.invoke(stream::iter(Vec::<InstanceInvokeRequest>::new())).await, Code::InvalidArgument).await?;
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
                        == Some(OperationPolicy {
                            retries: Some(RetryPolicy {
                                total_attempts: 3,
                                initial: Some(ProtoDuration { seconds: 1, nanos: 0 }),
                                maximum: Some(ProtoDuration { seconds: 4, nanos: 0 }),
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
            send.send(InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Cancel(Cancel { reason: "fixture_cancel".into() })),
            })
            .await
            .map_err(|error| Error::Invalid(error.to_string()))?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            require(frame == Some(instance_invoke_response::Frame::ChildCancel(ChildCancel { call: 1 })), "Missing Child Cancel")?;
            let result = terminal(&mut input).await?;
            let response::Result::Failure(failure) = result else {
                return Err(Error::Invalid("Expected Cancelled Failure".into()));
            };
            require(
                failure.classification == Classification::Cancelled as i32
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
                    == [Sample {
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
                InstanceSamplesRequest { frame: None },
                InstanceSamplesRequest {
                    frame: Some(instance_samples_request::Frame::Tick(Tick { at: Some(fixture::timestamp(1)) })),
                },
            ] {
                publish_status(client.samples(stream::iter([frame])).await, Code::InvalidArgument).await?;
            }
            publish_status(client.samples(stream::iter(Vec::<InstanceSamplesRequest>::new())).await, Code::InvalidArgument).await?;
            let open = InstanceSamplesRequest {
                frame: Some(instance_samples_request::Frame::Open(SamplesSelection {
                    stream: "conformance.ticks".into(),
                    scope: "fixture".into(),
                })),
            };
            publish_status(client.samples(stream::iter([open.clone(), open.clone()])).await, Code::InvalidArgument).await?;
            // Rule 14 / Reject Missing and Out-of-Range Tick Timestamps.
            for at in [
                None,
                Some(Timestamp { seconds: 253_402_300_800, nanos: 0 }),
                Some(Timestamp { seconds: 0, nanos: -1 }),
                Some(Timestamp { seconds: 0, nanos: 1_000_000_000 }),
            ] {
                let tick = InstanceSamplesRequest {
                    frame: Some(instance_samples_request::Frame::Tick(Tick { at })),
                };
                publish_status(client.samples(stream::iter([open.clone(), tick])).await, Code::InvalidArgument).await?;
            }
            let missing = InstanceSamplesRequest {
                frame: Some(instance_samples_request::Frame::Open(SamplesSelection {
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
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Missing Declarations".into()))?;
            let types = declarations.types.as_ref().ok_or_else(|| Error::Invalid("Missing Types".into()))?;
            let expected = fixture::declarations();
            require(declarations.operations == expected.operations, "Schema Operation Order or Contracts Differ")?;
            for operation in &expected.operations[6..] {
                let actual = declarations
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
            let actual = harmos_protocol::schema::message_types(types, "conformance.model.v1.Graph.Nested")
                .map_err(|error| Error::Invalid(error.to_string()))?;
            let expected = harmos_protocol::schema::message_types(&crate::conformance::types(), "conformance.model.v1.Graph.Nested")
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
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Missing Declarations".into()))?;
            let expected = fixture::declarations();
            require(
                declarations
                    .sections
                    .iter()
                    .filter(|section| section.r#type != "harmos.model.v1.SettingsDeclarations")
                    .eq(expected.sections.iter().filter(|section| section.r#type != "harmos.model.v1.SettingsDeclarations"))
                    && declarations.streams == expected.streams,
                "Interval Setting or Section Differs",
            )?;
            // Host Stream Settings / Answer Only the Host's Resolved Ticks.
            let (send, mut input) = open(&mut client, "conformance.settings").await?;
            tick(&send, &mut input, 1, 100).await?;
            tick(&send, &mut input, 2, 102).await?;
            drop(send);
            require(input.message().await?.is_none(), "Setting Observer Survived Demand Closure")?;
        }
        19 => {
            // Check Both Advertised Clock Requirements Against Fixed Fingerprints.
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Missing Declarations".into()))?;
            let types = declarations.types.as_ref().ok_or_else(|| Error::Invalid("Missing Types".into()))?;
            for (id, (request, response)) in fixture::CLOCK_OPERATIONS.into_iter().zip([
                ("harmos.host.v1.TimeSleepRequest", "harmos.host.v1.TimeSleepResponse"),
                ("harmos.host.v1.TimeGetRequest", "harmos.host.v1.TimeGetResponse"),
            ]) {
                let requirement = declarations
                    .requires
                    .iter()
                    .find(|item| item.id == id)
                    .ok_or_else(|| Error::Invalid(format!("Missing Clock Requirement: {id}")))?;
                let actual = harmos_protocol::schema::fingerprint(id, types, request, response)
                    .map_err(|error| Error::Invalid(error.to_string()))?;
                let host = harmos_protocol::schema::fingerprint(id, &harmos_protocol::clock::declarations_types(), request, response)
                    .map_err(|error| Error::Invalid(error.to_string()))?;
                let expected = fixture::fingerprint(id);
                require(requirement.fingerprint == expected && actual == expected && host == expected, "Clock Fingerprint Differs")?;
            }
            // Answer Host Clock Calls Without Any Local Timer.
            let (send, mut input) = invoke(&mut client, "conformance.clock", fixture::payload(&[])).await?;
            for (id, operation, payload, reply) in [
                (1, "harmos.host.v1.Time/Get", vec![], vec![0x0a, 2, 8, 100]),
                (2, "harmos.host.v1.Time/Sleep", vec![0x0a, 3, 8, 0x90, 0x1c], vec![]),
                (3, "harmos.host.v1.Time/Get", vec![], vec![0x0a, 3, 8, 0xf4, 0x1c]),
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
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Missing Declarations".into()))?;
            let name = "conformance.model.v1.PayloadOnly";
            require(
                !declarations.operations.iter().any(|item| item.request == name || item.response == name)
                    && !declarations.streams.iter().any(|item| item.row == name)
                    && !declarations.sections.iter().any(|item| item.r#type == name),
                "Payload Has An Executable Declaration",
            )?;
            let types = declarations.types.ok_or_else(|| Error::Invalid("Missing Types".into()))?;
            let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(types).map_err(|error| Error::Invalid(error.to_string()))?;
            let descriptor =
                pool.get_message_by_name(name).ok_or_else(|| Error::Invalid("Missing Independent Payload Descriptor".into()))?;
            let bytes = fixture::payload(b"record-only");
            let value =
                prost_reflect::DynamicMessage::decode(descriptor, bytes.as_slice()).map_err(|error| Error::Invalid(error.to_string()))?;
            require(value.encode_to_vec() == bytes, "Independent Payload Round Trip Differs")?;
        }
        21 => {
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Missing Declarations".into()))?;
            for (id, visibility, replay_safe) in [
                ("example.plugin.v1.Lifecycle/Initialize", Visibility::Protected, false),
                ("example.plugin.v1.Steps/Execute", Visibility::Public, true),
                ("conformance.step.0", Visibility::Public, false),
            ] {
                let operation = declarations
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
            send.send(InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Cancel(Cancel { reason: "Requested Stop".into() })),
            })
            .await
            .map_err(|_| Error::Invalid("Cancel Closed".into()))?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"Requested Stop")),
                "Cancellation Reason Did Not Reach Handler",
            )?;
        }
        24 => {
            let description = client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await?.into_inner();
            let declarations = description.declarations.ok_or_else(|| Error::Invalid("Declarations Missing".into()))?;
            require(
                !declarations.operations.iter().any(|operation| operation.id == "conformance.local"),
                "Private Operation Leaked Into Declarations",
            )?;
            require(
                !declarations.requires.iter().any(|operation| operation.id == "conformance.local"),
                "Private Requirement Leaked Into Declarations",
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
                    failure.classification = Classification::Refused as i32;
                    failure.message = format!(
                        "Invoke {operation}: {}",
                        if mode == b"protected" {
                            "Protected Peer Operation"
                        } else {
                            "Operation Is Not Declared in the Peer Declarations"
                        }
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
                    child(&mut input, call, "harmos.host.v1.Time/Get", &[]).await?;
                    send_child(
                        &send,
                        call,
                        child_response::Result::Payload(
                            TimeGetResponse {
                                now: Some(Timestamp { seconds: 0, nanos: 0 }),
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
                    require(child.operation == "harmos.host.v1.Time/Sleep", "Private Operation Leaked Onto Carrier")?;
                    let duration = TimeSleepRequest::decode(child.payload.as_slice())?.duration.unwrap();
                    if child.background {
                        require(duration == ProtoDuration { seconds: 2, nanos: 0 }, "Private Attempt Budget Differs")?;
                        deadline = Some(child.call);
                    } else {
                        require(duration == ProtoDuration { seconds: 3600, nanos: 0 }, "Private Handler Sleep Differs")?;
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
                        Some(instance_invoke_response::Frame::Response(Response {
                            result: Some(response::Result::Failure(failure)),
                        })) => {
                            require(
                                cancelled && failure.classification() == Classification::TimedOut,
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
            send.send(InstanceInvokeRequest {
                frame: Some(instance_invoke_request::Frame::Cancel(Cancel { reason: "Local Stop".into() })),
            })
            .await
            .map_err(|_| Error::Invalid("Private Cancel: Carrier Closed".into()))?;
            let mut guards = BTreeSet::new();
            loop {
                let frame = input.message().await?.and_then(|frame| frame.frame);
                match frame {
                    Some(instance_invoke_response::Frame::Child(child)) => {
                        require(
                            child.background && child.operation == "harmos.host.v1.Time/Sleep",
                            "Private Cancellation Leaked a Local Call",
                        )?;
                        guards.insert(child.call);
                    }
                    Some(instance_invoke_response::Frame::ChildCancel(child)) => {
                        guards.remove(&child.call);
                    }
                    Some(instance_invoke_response::Frame::Response(Response {
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
            let delay = TimeSleepRequest {
                duration: Some(ProtoDuration { seconds: 1, nanos: 11 }),
            }
            .encode_to_vec();
            child(&mut input, 1, "harmos.host.v1.Time/Sleep", &delay).await?;
            send_child(&send, 1, child_response::Result::Payload(Vec::new())).await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"2")),
                "Private Retry Did Not Preserve Attempt Semantics",
            )?;
            // An Hour-Long Retry Must Stop at the Remaining Overall Budget.
            let (send, mut input) = invoke(&mut client, "conformance.local_call", fixture::payload(b"retry-bound")).await?;
            let mut now = 0;
            let mut slept = false;
            let mut guards = BTreeSet::new();
            loop {
                let frame = input.message().await?.and_then(|frame| frame.frame);
                match frame {
                    Some(instance_invoke_response::Frame::Child(child)) => {
                        require(child.fingerprint == fixture::fingerprint(&child.operation), "Private Retry: Clock Fingerprint Differs")?;
                        if child.operation == "harmos.host.v1.Time/Get" {
                            send_child(
                                &send,
                                child.call,
                                child_response::Result::Payload(
                                    TimeGetResponse {
                                        now: Some(Timestamp { seconds: now, nanos: 0 }),
                                    }
                                    .encode_to_vec(),
                                ),
                            )
                            .await?;
                        } else {
                            require(child.operation == "harmos.host.v1.Time/Sleep", "Private Retry Leaked Onto the Carrier")?;
                            let duration = TimeSleepRequest::decode(child.payload.as_slice())?.duration.unwrap();
                            require(duration == ProtoDuration { seconds: 1, nanos: 0 }, "Retry Outlived the Overall Deadline")?;
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
                    Some(instance_invoke_response::Frame::Response(Response {
                        result: Some(response::Result::Failure(failure)),
                    })) => {
                        require(
                            slept && guards.is_empty() && failure.classification() == Classification::TimedOut,
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
                        frame == Some(instance_invoke_response::Frame::ChildDemand(ChildDemand { call })),
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
                            == Some(instance_invoke_response::Frame::ChildCancel(ChildCancel { call })),
                        "Watch Demand Not Cancelled",
                    )?;
                    require(
                        terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"first")),
                        "Watch Cancel Result Differs",
                    )?;
                } else {
                    require(
                        input.message().await?.and_then(|frame| frame.frame)
                            == Some(instance_invoke_response::Frame::ChildDemand(ChildDemand { call })),
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
            client.stop(InstanceStopRequest { reason: "Conformance Complete".into() }).await?;
            if !managed {
                connect(address).await?;
            }
        }
        _ => return Err(Error::Invalid("Unknown Rule".into())),
    }
    Ok(())
}

/* === Tests === */
