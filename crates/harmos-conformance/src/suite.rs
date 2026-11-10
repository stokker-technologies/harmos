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
use tokio::sync::mpsc;
use tonic::{Code, Streaming, transport::Channel};

// Local Imports
use crate::{Error, Result, fixture};
use harmos_protocol::v1::{self as v1, child_response, host_frame, plugin_client::PluginClient, plugin_frame, publish_frame, response};

/* === Constants === */

pub const CASES: &[&str] = &[
    "Describe and Catalog",
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
    "Publish Frame Validation",
    "Child Correlation Validation",
    "Stop and Cleanup",
];

static INVOCATION: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

type Client = PluginClient<Channel>;
type Invoke = (mpsc::Sender<v1::HostFrame>, Streaming<v1::PluginFrame>);
type Publish = (mpsc::Sender<v1::PublishFrame>, Streaming<v1::Samples>);

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
    for (index, name) in CASES.iter().enumerate() {
        let result = tokio::time::timeout(Duration::from_secs(30), case(index + 1, address, managed)).await;
        let error = match result {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some("Case Hang Guard Expired".into()),
        };
        results.push(Case { rule: index + 1, name, error });
    }
    Ok(results)
}

/* === Private Functions === */

async fn connect(address: &str) -> Result<(Client, v1::Description)> {
    let mut client = Client::connect(address.to_owned()).await?;
    let description = describe(&mut client).await?;
    Ok((client, description))
}

async fn describe(client: &mut Client) -> Result<v1::Description> {
    Ok(client
        .describe(v1::DescribeRequest {
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

fn request(operation: &str, payload: Vec<u8>) -> v1::HostFrame {
    let invocation = INVOCATION.fetch_add(1, Ordering::Relaxed);
    v1::HostFrame {
        frame: Some(host_frame::Frame::Request(v1::Request {
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

async fn terminal(input: &mut Streaming<v1::PluginFrame>) -> Result<response::Result> {
    let frame = input.message().await?.and_then(|frame| frame.frame);
    let Some(plugin_frame::Frame::Response(v1::Response { result: Some(result) })) = frame else {
        return Err(Error::Invalid(format!("Expected Terminal Response, Got {frame:?}")));
    };
    require(input.message().await?.is_none(), "Frames After Terminal Response")?;
    Ok(result)
}

async fn child(input: &mut Streaming<v1::PluginFrame>, id: u64, operation: &str, payload: &[u8]) -> Result<()> {
    let frame = input.message().await?.and_then(|frame| frame.frame);
    let Some(plugin_frame::Frame::Child(child)) = frame else {
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

async fn send_child(send: &mpsc::Sender<v1::HostFrame>, call: u64, result: child_response::Result) -> Result<()> {
    send.send(v1::HostFrame {
        frame: Some(host_frame::Frame::Child(v1::ChildResponse { call, result: Some(result) })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))
}

async fn open(client: &mut Client, name: &str) -> Result<Publish> {
    let (send, receive) = mpsc::channel(8);
    send.send(v1::PublishFrame {
        frame: Some(publish_frame::Frame::Open(v1::PublishRequest {
            stream: name.into(),
            scope: "fixture".into(),
        })),
    })
    .await
    .map_err(|error| Error::Invalid(error.to_string()))?;
    let outgoing = stream::unfold(receive, |mut receive| async { receive.recv().await.map(|frame| (frame, receive)) });
    let incoming = client.publish(outgoing).await?.into_inner();
    Ok((send, incoming))
}

async fn tick(send: &mpsc::Sender<v1::PublishFrame>, input: &mut Streaming<v1::Samples>, sequence: u64, seconds: i64) -> Result<()> {
    let at = fixture::timestamp(seconds);
    send.send(v1::PublishFrame {
        frame: Some(publish_frame::Frame::Tick(v1::Tick { at: Some(at) })),
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
    result: std::result::Result<tonic::Response<Streaming<v1::PluginFrame>>, tonic::Status>, expected: Code,
) -> Result<()> {
    let status = match result {
        Err(status) => status,
        Ok(response) => response.into_inner().message().await.err().ok_or_else(|| Error::Invalid("Expected gRPC Error".into()))?,
    };
    require(status.code() == expected, &format!("Expected {expected:?}, Got {status}"))
}

async fn publish_status(result: std::result::Result<tonic::Response<Streaming<v1::Samples>>, tonic::Status>, expected: Code) -> Result<()> {
    let status = match result {
        Err(status) => status,
        Ok(response) => response.into_inner().message().await.err().ok_or_else(|| Error::Invalid("Expected gRPC Error".into()))?,
    };
    require(status.code() == expected, &format!("Expected {expected:?}, Got {status}"))
}

async fn case(rule: usize, address: &str, managed: bool) -> Result<()> {
    let (mut client, description) = connect(address).await?;
    match rule {
        1 => {
            require(
                description.id == "conformance"
                    && description.version == "1.0.0"
                    && description.revision == 1
                    && !description.sdk.is_empty()
                    && !description.sdk_version.is_empty(),
                "Description Identity Missing",
            )?;
            let catalog = description.catalog.ok_or_else(|| Error::Invalid("Missing Catalog".into()))?;
            require(catalog == fixture::catalog(), "Fixture Catalog Differs")?;
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
                let (send, mut input) = invoke(&mut client, "conformance.echo", payload.clone()).await?;
                drop(send);
                require(terminal(&mut input).await? == response::Result::Payload(payload), "Echo Changed Protobuf Payload")?;
            }
        }
        4 => {
            for frame in [
                v1::HostFrame { frame: None },
                v1::HostFrame {
                    frame: Some(host_frame::Frame::Cancel(v1::Cancel { reason: "early".into() })),
                },
                v1::HostFrame {
                    frame: Some(host_frame::Frame::Child(v1::ChildResponse::default())),
                },
            ] {
                invoke_status(client.invoke(stream::iter([frame])).await, Code::InvalidArgument).await?;
            }
            invoke_status(client.invoke(stream::iter(Vec::<v1::HostFrame>::new())).await, Code::InvalidArgument).await?;
            let frames = [request("conformance.cancel", fixture::payload(b"")), request("conformance.echo", fixture::payload(b""))];
            let mut input = client.invoke(stream::iter(frames)).await?.into_inner();
            child(&mut input, 1, "conformance.host.pending", &fixture::payload(b"")).await?;
            require(input.message().await.err().is_some_and(|status| status.code() == Code::InvalidArgument), "Repeated Request Accepted")?;
        }
        5 => {
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
            send.send(v1::HostFrame {
                frame: Some(host_frame::Frame::Cancel(v1::Cancel { reason: "fixture_cancel".into() })),
            })
            .await
            .map_err(|error| Error::Invalid(error.to_string()))?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            require(frame == Some(plugin_frame::Frame::ChildCancel(v1::ChildCancel { call: 1 })), "Missing Child Cancel")?;
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
            let mut frame = request("conformance.echo", vec![255]);
            invoke_status(client.invoke(stream::iter([frame.clone()])).await, Code::InvalidArgument).await?;
            if let Some(host_frame::Frame::Request(request)) = &mut frame.frame {
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
            let (_send, mut input) = invoke(&mut client, "conformance.echo", fixture::payload(b"independent")).await?;
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
            let (_send, mut input) = invoke(&mut client, "conformance.echo", fixture::payload(b"after close")).await?;
            require(
                terminal(&mut input).await? == response::Result::Payload(fixture::payload(b"after close")),
                "Closed Demand Broke Connection",
            )?;
        }
        14 => {
            for frame in [
                v1::PublishFrame { frame: None },
                v1::PublishFrame {
                    frame: Some(publish_frame::Frame::Tick(v1::Tick { at: Some(fixture::timestamp(1)) })),
                },
            ] {
                publish_status(client.publish(stream::iter([frame])).await, Code::InvalidArgument).await?;
            }
            publish_status(client.publish(stream::iter(Vec::<v1::PublishFrame>::new())).await, Code::InvalidArgument).await?;
            let open = v1::PublishFrame {
                frame: Some(publish_frame::Frame::Open(v1::PublishRequest {
                    stream: "conformance.ticks".into(),
                    scope: "fixture".into(),
                })),
            };
            publish_status(client.publish(stream::iter([open.clone(), open.clone()])).await, Code::InvalidArgument).await?;
            let tick = v1::PublishFrame {
                frame: Some(publish_frame::Frame::Tick(v1::Tick { at: None })),
            };
            publish_status(client.publish(stream::iter([open, tick])).await, Code::InvalidArgument).await?;
            let missing = v1::PublishFrame {
                frame: Some(publish_frame::Frame::Open(v1::PublishRequest {
                    stream: "conformance.absent".into(),
                    scope: "fixture".into(),
                })),
            };
            publish_status(client.publish(stream::iter([missing])).await, Code::NotFound).await?;
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
        16 => {
            client.stop(v1::StopRequest { reason: "Conformance Complete".into() }).await?;
            if !managed {
                connect(address).await?;
            }
        }
        _ => return Err(Error::Invalid("Unknown Rule".into())),
    }
    Ok(())
}

/* === Tests === */
