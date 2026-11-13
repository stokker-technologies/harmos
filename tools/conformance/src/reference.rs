/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Hand-Written Tonic Plugin Without SDK or Framed Transport.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{io::Write, pin::Pin, sync::Arc, time::SystemTime};

// Third Party Imports
use futures_core::Stream;
use futures_util::stream;
use prost::Message;
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc, oneshot},
};
use tonic::{Request, Response, Status, Streaming};

// Local Imports
use crate::{
    Result, fixture,
    schema_fixture::{Graph, Nested},
};
use harmos_protocol::plugin::{
    instance_invoke_request, instance_invoke_response, instance_samples_request,
    instance_server::{Instance, InstanceServer},
};
use harmos_protocol::v1::{self as v1, child_response, response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = std::result::Result<T, Status>> + Send>>;

/* === Structs === */

struct Reference {
    started_at: prost_types::Timestamp,
    shutdown: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[tonic::async_trait]
impl Instance for Reference {
    async fn get(
        &self, _: Request<harmos_protocol::plugin::InstanceGetRequest>,
    ) -> std::result::Result<Response<harmos_protocol::plugin::InstanceGetResponse>, Status> {
        Ok(Response::new(harmos_protocol::plugin::InstanceGetResponse {
            message_bytes: harmos_protocol::transfer::default_message_bytes().get() as u64,
            id: "conformance".into(),
            version: "1.0.0".into(),
            started_at: Some(self.started_at),
            catalog: Some(fixture::catalog()),
            revision: 1,
            sdk: "hand_written_tonic".into(),
            sdk_version: "1.0.0".into(),
        }))
    }

    type InvokeStream = Output<harmos_protocol::plugin::InstanceInvokeResponse>;

    async fn invoke(
        &self, request: Request<Streaming<harmos_protocol::plugin::InstanceInvokeRequest>>,
    ) -> std::result::Result<Response<Self::InvokeStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(instance_invoke_request::Frame::Request(request)) = first else {
            return Err(Status::invalid_argument("Request Must Be First"));
        };
        if request.invocation.is_empty() || request.attempt == 0 {
            return Err(Status::invalid_argument("Invocation and Attempt Are Required"));
        }
        match request.operation.as_str() {
            "conformance.schema" | "conformance.projected" => {
                Graph::decode(request.payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Graph"))?;
            }
            "conformance.nested" => {
                Nested::decode(request.payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Nested"))?;
            }
            _ => {
                fixture::Value::decode(request.payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Payload"))?;
            }
        }
        let (output, receiver) = mpsc::channel(8);
        tokio::spawn(async move {
            let result = invoke(request, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(Box::pin(stream::unfold(
            receiver,
            |mut receiver| async move { receiver.recv().await.map(|item| (item, receiver)) },
        ))))
    }

    type SamplesStream = Output<harmos_protocol::plugin::InstanceSamplesResponse>;

    async fn samples(
        &self, request: Request<Streaming<harmos_protocol::plugin::InstanceSamplesRequest>>,
    ) -> std::result::Result<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(instance_samples_request::Frame::Open(open)) = first else {
            return Err(Status::invalid_argument("Open Must Be First"));
        };
        if !["conformance.ticks", "conformance.device", "conformance.settings"].contains(&open.stream.as_str()) {
            return Err(Status::not_found("Unknown Stream"));
        }
        let (output, receiver) = mpsc::channel(8);
        tokio::spawn(async move {
            let result = samples(open, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(Box::pin(stream::unfold(
            receiver,
            |mut receiver| async move { receiver.recv().await.map(|item| (item, receiver)) },
        ))))
    }

    async fn stop(
        &self, _: Request<harmos_protocol::plugin::InstanceStopRequest>,
    ) -> std::result::Result<Response<harmos_protocol::plugin::InstanceStopResponse>, Status> {
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        Ok(Response::new(harmos_protocol::plugin::InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub async fn serve(remote: bool) -> Result<()> {
    let address = std::env::var("HARMOS_ADDRESS").unwrap_or_else(|_| "127.0.0.1:0".into());
    let listener = TcpListener::bind(address).await?;
    let started_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_err(|error| crate::Error::Invalid(error.to_string()))?;
    let (shutdown, stopped) = oneshot::channel();
    let reference = Reference {
        started_at: prost_types::Timestamp {
            seconds: started_at.as_secs() as i64,
            nanos: started_at.subsec_nanos() as i32,
        },
        shutdown: Arc::new(Mutex::new((!remote).then_some(shutdown))),
    };
    let address = listener.local_addr()?;
    let incoming = stream::unfold(listener, |listener| async {
        let accepted = listener.accept().await.map(|(socket, _)| socket);
        Some((accepted, listener))
    });
    println!("harmos listening {address}");
    std::io::stdout().flush()?;
    tonic::transport::Server::builder()
        .add_service(InstanceServer::new(reference).max_decoding_message_size(harmos_protocol::transfer::default_message_bytes().get()).max_encoding_message_size(harmos_protocol::transfer::default_message_bytes().get()))
        .serve_with_incoming_shutdown(incoming, async move {
            if remote {
                std::future::pending::<()>().await;
            }
            let _ = stopped.await;
        })
        .await?;
    Ok(())
}

/* === Private Functions === */

async fn invoke(
    request: v1::Request, input: &mut Streaming<harmos_protocol::plugin::InstanceInvokeRequest>,
    output: &mpsc::Sender<std::result::Result<harmos_protocol::plugin::InstanceInvokeResponse, Status>>,
) -> std::result::Result<(), Status> {
    let result = match request.operation.as_str() {
        "conformance.watch" => {
            output
                .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                    frame: Some(instance_invoke_response::Frame::Child(v1::ChildRequest {
                        call: 1,
                        operation: "conformance.host.watch".into(),
                        payload: request.payload.clone(),
                        fingerprint: fixture::fingerprint("conformance.host.watch"),
                        watch: true,
                        max_response_bytes: 4096,
                        ..Default::default()
                    })),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            let cancel = fixture::Value::decode(request.payload.as_slice()).unwrap().value == b"cancel";
            let mut value = Vec::new();
            loop {
                output
                    .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                        frame: Some(instance_invoke_response::Frame::ChildDemand(v1::ChildDemand { call: 1 })),
                    }))
                    .await
                    .map_err(|_| Status::cancelled("Host Closed"))?;
                let Some(instance_invoke_request::Frame::Child(child)) = input.message().await?.and_then(|frame| frame.frame) else {
                    return Err(Status::invalid_argument("Watch: Expected Child Reply"));
                };
                match child.result {
                    Some(child_response::Result::Item(item)) => {
                        value.extend(fixture::Value::decode(item.as_slice()).unwrap().value);
                        if cancel {
                            output
                                .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                                    frame: Some(instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call: 1 })),
                                }))
                                .await
                                .map_err(|_| Status::cancelled("Host Closed"))?;
                            break response::Result::Payload(fixture::payload(&value));
                        }
                    }
                    Some(child_response::Result::Payload(_)) => break response::Result::Payload(fixture::payload(&value)),
                    Some(child_response::Result::Failure(failure)) => break response::Result::Failure(failure),
                    None => return Err(Status::invalid_argument("Watch: Empty Reply")),
                }
            }
        }
        "conformance.peer_call" => {
            let mode = fixture::Value::decode(request.payload.as_slice()).unwrap().value;
            let operation = if mode == b"public" {
                "conformance.peer.public"
            } else if mode == b"self" {
                "example.plugin.v1.Lifecycle/Initialize"
            } else if mode == b"protected" {
                "conformance.peer.protected"
            } else {
                "conformance.peer.local"
            };
            let payload = if mode == b"public" { fixture::payload(request.scope.as_bytes()) } else { request.payload.clone() };
            send_child(output, 1, operation, payload).await?;
            let Some(instance_invoke_request::Frame::Child(child)) = input.message().await?.and_then(|frame| frame.frame) else {
                return Err(Status::invalid_argument("Invoke Peer: Expected Child Reply"));
            };
            match child.result {
                Some(child_response::Result::Payload(payload)) => response::Result::Payload(payload),
                Some(child_response::Result::Failure(failure)) => response::Result::Failure(failure),
                _ => return Err(Status::invalid_argument("Invoke Peer: Missing Result")),
            }
        }
        "conformance.local_call" => {
            let value = fixture::Value::decode(request.payload.as_slice()).unwrap().value;
            if matches!(value.as_slice(), b"deadline" | b"overall") {
                for call in 1..=2 {
                    send_child(output, call, "harmos.context.v1.Time/Get", Vec::new()).await?;
                    input.message().await?;
                }
                for (call, seconds, background) in [(3, 2, true), (4, 3600, false)] {
                    output
                        .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                            frame: Some(instance_invoke_response::Frame::Child(v1::ChildRequest {
                                call,
                                operation: "harmos.context.v1.Time/Sleep".into(),
                                payload: harmos_protocol::context::TimeSleepRequest {
                                    duration: Some(prost_types::Duration { seconds, nanos: 0 }),
                                }
                                .encode_to_vec(),
                                fingerprint: fixture::fingerprint("harmos.context.v1.Time/Sleep"),
                                background,
                                ..Default::default()
                            })),
                        }))
                        .await
                        .map_err(|_| Status::cancelled("Host Closed"))?;
                }
                input.message().await?;
                output
                    .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                        frame: Some(instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call: 4 })),
                    }))
                    .await
                    .map_err(|_| Status::cancelled("Host Closed"))?;
                response::Result::Failure(v1::Failure {
                    classification: v1::Classification::TimedOut as i32,
                    code: "deadline".into(),
                    message: "Invoke: Deadline Reached".into(),
                    ..Default::default()
                })
            } else if value == b"cancel" {
                send_child(output, 1, "conformance.host.echo", request.payload.clone()).await?;
                input.message().await?;
                let Some(instance_invoke_request::Frame::Cancel(cancel)) = input.message().await?.and_then(|frame| frame.frame) else {
                    return Err(Status::invalid_argument("Private Cancel: Expected Cancel"));
                };
                response::Result::Payload(fixture::payload(cancel.reason.as_bytes()))
            } else if value == b"retry-bound" {
                send_child(output, 1, "harmos.context.v1.Time/Get", Vec::new()).await?;
                input.message().await?;
                send_child(
                    output,
                    2,
                    "harmos.context.v1.Time/Sleep",
                    harmos_protocol::context::TimeSleepRequest {
                        duration: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                    }
                    .encode_to_vec(),
                )
                .await?;
                input.message().await?;
                response::Result::Failure(v1::Failure {
                    classification: v1::Classification::TimedOut as i32,
                    code: "deadline".into(),
                    message: "Invoke Local: Overall Deadline Reached".into(),
                    ..Default::default()
                })
            } else if value == b"retry" {
                send_child(
                    output,
                    1,
                    "harmos.context.v1.Time/Sleep",
                    harmos_protocol::context::TimeSleepRequest {
                        duration: Some(prost_types::Duration { seconds: 1, nanos: 11 }),
                    }
                    .encode_to_vec(),
                )
                .await?;
                input.message().await?;
                response::Result::Payload(fixture::payload(b"2"))
            } else {
                response::Result::Payload(request.payload.clone())
            }
        }
        "conformance.clock" => {
            let mut readings = Vec::new();
            for (call, operation, payload) in [
                (1, "harmos.context.v1.Time/Get", vec![]),
                (
                    2,
                    "harmos.context.v1.Time/Sleep",
                    harmos_protocol::context::TimeSleepRequest {
                        duration: Some(prost_types::Duration { seconds: 3600, nanos: 0 }),
                    }
                    .encode_to_vec(),
                ),
                (3, "harmos.context.v1.Time/Get", vec![]),
            ] {
                send_child(output, call, operation, payload).await?;
                let Some(instance_invoke_request::Frame::Child(v1::ChildResponse {
                    call: received,
                    result: Some(child_response::Result::Payload(payload)),
                })) = input.message().await?.and_then(|frame| frame.frame)
                else {
                    return Err(Status::invalid_argument("Expected Clock Reply"));
                };
                if received != call {
                    return Err(Status::invalid_argument("Unexpected Clock Reply"));
                }
                if operation == "harmos.context.v1.Time/Get" {
                    readings.push(
                        harmos_protocol::context::TimeGetResponse::decode(payload.as_slice())
                            .map_err(|_| Status::invalid_argument("Invalid Time"))?
                            .now
                            .ok_or_else(|| Status::invalid_argument("Missing Time"))?
                            .seconds,
                    );
                }
            }
            response::Result::Payload(fixture::payload(&(readings[1] - readings[0]).to_le_bytes()))
        }
        "conformance.projected" => {
            let graph = Graph::decode(request.payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Graph"))?;
            response::Result::Payload(graph.tail.and_then(|tail| tail.value).unwrap_or_default().encode_to_vec())
        }
        "conformance.policy" => {
            let child = v1::ChildRequest {
                call: 1,
                operation: "conformance.host.echo".into(),
                payload: request.payload,
                fingerprint: fixture::fingerprint("conformance.host.echo"),
                policy: Some(v1::OperationPolicy {
                    retries: Some(harmos_protocol::v1::RetryPolicy {
                        total_attempts: 3,
                        initial: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
                        maximum: Some(prost_types::Duration { seconds: 4, nanos: 0 }),
                        deterministic_jitter: true,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            };
            output
                .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                    frame: Some(instance_invoke_response::Frame::Child(child)),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            let Some(instance_invoke_request::Frame::Child(v1::ChildResponse {
                call: 1,
                result: Some(child_response::Result::Payload(payload)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Expected First Child Reply"));
            };
            send_child(output, 2, "conformance.host.echo", payload).await?;
            let Some(instance_invoke_request::Frame::Child(v1::ChildResponse {
                call: 2,
                result: Some(child_response::Result::Payload(payload)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Expected Second Child Reply"));
            };
            response::Result::Payload(payload)
        }
        "conformance.schema" => {
            let graph = Graph::decode(request.payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Graph"))?;
            response::Result::Payload(
                Nested {
                    count: Some(graph.items.len() as i32),
                    next: Some(Box::new(graph)),
                }
                .encode_to_vec(),
            )
        }
        "conformance.nested" => {
            let nested = Nested::decode(request.payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Nested"))?;
            response::Result::Payload(nested.next.unwrap_or_default().encode_to_vec())
        }
        "conformance.step.0" => {
            if request.payload == fixture::payload(b"scope") {
                if request.scope.is_empty() {
                    response::Result::Failure(v1::Failure {
                        classification: v1::Classification::Refused as i32,
                        code: "scope_required".into(),
                        message: "Invocation Scope Is Required: the caller must bind a scope".into(),
                        ..Default::default()
                    })
                } else {
                    response::Result::Payload(fixture::payload(request.scope.as_bytes()))
                }
            } else {
                response::Result::Payload(request.payload)
            }
        }
        "example.plugin.v1.Lifecycle/Initialize" => response::Result::Payload(request.payload),
        "example.plugin.v1.Steps/Execute" | "conformance.cancel_reason" => {
            send_child(output, 1, "conformance.host.echo", request.payload.clone()).await?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            if !matches!(
                frame,
                Some(instance_invoke_request::Frame::Child(v1::ChildResponse {
                    call: 1,
                    result: Some(child_response::Result::Payload(_))
                }))
            ) {
                return Err(Status::invalid_argument("Expected Resource Child Reply"));
            }
            if request.operation == "example.plugin.v1.Steps/Execute" {
                response::Result::Payload(request.payload)
            } else {
                let Some(instance_invoke_request::Frame::Cancel(cancel)) = input.message().await?.and_then(|frame| frame.frame) else {
                    return Err(Status::invalid_argument("Expected Cancel Reason"));
                };
                response::Result::Payload(fixture::payload(cancel.reason.as_bytes()))
            }
        }
        "conformance.failure" => response::Result::Failure(fixture::failure("conformance.failure")),
        "conformance.children" => {
            for call in [1, 2] {
                send_child(output, call, "conformance.host.echo", fixture::payload(&[call as u8])).await?;
            }
            let mut replies = std::collections::BTreeMap::new();
            while replies.len() < 2 {
                let frame = input.message().await?.and_then(|frame| frame.frame);
                let Some(instance_invoke_request::Frame::Child(child)) = frame else {
                    return Err(Status::invalid_argument("Expected Child Response"));
                };
                if ![1, 2].contains(&child.call) || replies.contains_key(&child.call) {
                    return Err(Status::invalid_argument("Unknown or Duplicate Child"));
                }
                let Some(child_response::Result::Payload(payload)) = child.result else {
                    return Err(Status::invalid_argument("Expected Child Payload"));
                };
                let value = fixture::Value::decode(payload.as_slice()).map_err(|_| Status::invalid_argument("Invalid Child Payload"))?;
                replies.insert(child.call, value.value);
            }
            let value = replies.into_values().flatten().collect::<Vec<_>>();
            response::Result::Payload(fixture::payload(&value))
        }
        "conformance.child_failure" => {
            send_child(output, 1, "conformance.host.fail", request.payload).await?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            let Some(instance_invoke_request::Frame::Child(v1::ChildResponse {
                call: 1,
                result: Some(child_response::Result::Failure(failure)),
            })) = frame
            else {
                return Err(Status::invalid_argument("Expected Child Failure"));
            };
            response::Result::Failure(failure)
        }
        "conformance.cancel" => {
            send_child(output, 1, "conformance.host.pending", request.payload).await?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            let Some(instance_invoke_request::Frame::Cancel(cancel)) = frame else {
                return Err(Status::invalid_argument("Expected Cancel"));
            };
            output
                .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
                    frame: Some(instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call: 1 })),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            response::Result::Failure(v1::Failure {
                classification: v1::Classification::Cancelled as i32,
                code: "cancelled".into(),
                message: cancel.reason,
                action: "Retry If Needed".into(),
                operation: request.operation,
                plugin: "conformance".into(),
                ..Default::default()
            })
        }
        _ => return Err(Status::not_found("Unknown Operation")),
    };
    output
        .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
            frame: Some(instance_invoke_response::Frame::Response(v1::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn send_child(
    output: &mpsc::Sender<std::result::Result<harmos_protocol::plugin::InstanceInvokeResponse, Status>>, call: u64, operation: &str,
    payload: Vec<u8>,
) -> std::result::Result<(), Status> {
    let child = v1::ChildRequest {
        call,
        operation: operation.into(),
        payload,
        fingerprint: fixture::fingerprint(operation),
        policy: None,
        ..Default::default()
    };
    output
        .send(Ok(harmos_protocol::plugin::InstanceInvokeResponse {
            frame: Some(instance_invoke_response::Frame::Child(child)),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))
}

async fn samples(
    open: v1::SamplesSelection, input: &mut Streaming<harmos_protocol::plugin::InstanceSamplesRequest>,
    output: &mpsc::Sender<std::result::Result<harmos_protocol::plugin::InstanceSamplesResponse, Status>>,
) -> std::result::Result<(), Status> {
    if open.stream == "conformance.device" {
        let samples = harmos_protocol::plugin::InstanceSamplesResponse {
            samples: vec![v1::Sample {
                at: None,
                row: fixture::payload(b"device"),
                sequence: 1,
            }],
        };
        output.send(Ok(samples)).await.map_err(|_| Status::cancelled("Host Closed"))?;
    }
    let mut sequence = 0_u64;
    loop {
        let frame = tokio::select! {
            _ = output.closed() => return Ok(()),
            frame = input.message() => frame?,
        };
        let Some(frame) = frame else {
            return Ok(());
        };
        let Some(instance_samples_request::Frame::Tick(tick)) = frame.frame else {
            return Err(Status::invalid_argument("Expected Tick"));
        };
        let Some(at) = tick.at else {
            return Err(Status::invalid_argument("Tick Time Required"));
        };
        if open.stream == "conformance.device"
            || at.nanos < 0
            || at.nanos >= 1_000_000_000
            || !(-62_135_596_800..=253_402_300_799).contains(&at.seconds)
        {
            return Err(Status::invalid_argument("Invalid Tick"));
        }
        sequence += 1;
        let row = fixture::payload(&sequence.to_le_bytes());
        let samples = harmos_protocol::plugin::InstanceSamplesResponse {
            samples: vec![v1::Sample { at: Some(at), row, sequence }],
        };
        output.send(Ok(samples)).await.map_err(|_| Status::cancelled("Host Closed"))?;
    }
}

/* === Tests === */
