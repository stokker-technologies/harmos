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
use std::collections::BTreeMap;
use std::result;
use std::{io::Write, pin::Pin, sync::Arc, time::SystemTime};

// Third Party Imports
use futures_core::Stream;
use futures_util::stream;
use prost::Message;
use prost_types::{Duration, Timestamp};
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc, oneshot},
};
use tonic::{Request, Response, Status, Streaming};

// Local Imports
use crate::conformance::{Graph, graph::Nested};
use crate::{Result, fixture};
use harmos_protocol::host::plugins::SettingsListResponse;
use harmos_protocol::host::{OperationsGetRequest, OperationsGetResponse, OperationsListResponse, TimeGetResponse, TimeSleepRequest};
use harmos_protocol::model::RetryPolicy;
use harmos_protocol::model::{Classification, Failure, OperationPolicy};
use harmos_protocol::plugin;
use harmos_protocol::plugin::{ChildCancel, ChildDemand, ChildRequest, ChildResponse, Sample, SampleBatch, SamplesSelection};
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
    InstanceSamplesResponse, InstanceStopRequest, InstanceStopResponse,
};
use harmos_protocol::plugin::{child_response, response};
use harmos_protocol::plugin::{
    instance_invoke_request, instance_invoke_response, instance_samples_request, instance_samples_response,
    instance_server::{Instance, InstanceServer},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = result::Result<T, Status>> + Send>>;

/* === Structs === */

struct Reference {
    started_at: Timestamp,
    shutdown: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[tonic::async_trait]
impl Instance for Reference {
    async fn get(&self, _: Request<InstanceGetRequest>) -> result::Result<Response<InstanceGetResponse>, Status> {
        Ok(Response::new(InstanceGetResponse {
            message_bytes: harmos_protocol::transfer::default_message_bytes().get() as u64,
            id: "conformance".into(),
            version: "1.0.0".into(),
            started_at: Some(self.started_at),
            declarations: Some(fixture::declarations()),
            revision: 1,
            sdk: "hand_written_tonic".into(),
            sdk_version: "1.0.0".into(),
        }))
    }

    type InvokeStream = Output<InstanceInvokeResponse>;

    async fn invoke(&self, request: Request<Streaming<InstanceInvokeRequest>>) -> result::Result<Response<Self::InvokeStream>, Status> {
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

    type SamplesStream = Output<InstanceSamplesResponse>;

    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> result::Result<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(instance_samples_request::Frame::Open(open)) = first else {
            return Err(Status::invalid_argument("Open Must Be First"));
        };
        if !["conformance.interval", "conformance.device", "conformance.settings", fixture::WINDOW, fixture::OVERSIZED]
            .contains(&open.stream.as_str())
            && !fixture::WINDOW_STREAMS.contains(&open.stream.as_str())
        {
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

    async fn stop(&self, _: Request<InstanceStopRequest>) -> result::Result<Response<InstanceStopResponse>, Status> {
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        Ok(Response::new(InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub async fn serve(remote: bool) -> Result<()> {
    let address = std::env::var("HARMOS_ADDRESS").unwrap_or_else(|_| "127.0.0.1:0".into());
    let listener = TcpListener::bind(address).await?;
    let started_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_err(|error| crate::Error::Invalid(error.to_string()))?;
    let (shutdown, stopped) = oneshot::channel();
    let reference = Reference {
        started_at: Timestamp {
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
        .add_service(
            InstanceServer::new(reference)
                .max_decoding_message_size(harmos_protocol::transfer::default_message_bytes().get())
                .max_encoding_message_size(harmos_protocol::transfer::default_message_bytes().get()),
        )
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
    request: plugin::Request, input: &mut Streaming<InstanceInvokeRequest>,
    output: &mpsc::Sender<result::Result<InstanceInvokeResponse, Status>>,
) -> result::Result<(), Status> {
    let result = match request.operation.as_str() {
        "conformance.caller_refusal" => {
            send_child(output, 1, "application.host.v1.Invocations/Create", crate::callers::request()).await?;
            let Some(instance_invoke_request::Frame::Child(ChildResponse {
                call: 1,
                result: Some(child_response::Result::Failure(failure)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Invoke Client Operation: Expected Caller Refusal"));
            };
            let value = fixture::Value::decode(request.payload.as_slice()).map_err(|error| Status::invalid_argument(error.to_string()))?;
            if value.value == b"propagate" {
                response::Result::Failure(failure)
            } else {
                response::Result::Payload(fixture::payload(&failure.encode_to_vec()))
            }
        }
        "conformance.settings_read" => {
            send_child(output, 1, "harmos.plugin.v1.plugins.Settings/List", Vec::new()).await?;
            let Some(instance_invoke_request::Frame::Child(child)) = input.message().await?.and_then(|frame| frame.frame) else {
                return Err(Status::invalid_argument("List Settings: Expected Reply"));
            };
            let Some(child_response::Result::Payload(payload)) = child.result else {
                return Err(Status::invalid_argument("List Settings: Expected Values"));
            };
            let response =
                SettingsListResponse::decode(payload.as_slice()).map_err(|_| Status::invalid_argument("List Settings: Invalid Values"))?;
            response::Result::Payload(fixture::payload(
                &response.value.ok_or_else(|| Status::invalid_argument("List Settings: Missing Values"))?.encode_to_vec(),
            ))
        }
        "conformance.discover" => {
            send_child(output, 1, "harmos.plugin.v1.Operations/List", Vec::new()).await?;
            let Some(instance_invoke_request::Frame::Child(child)) = input.message().await?.and_then(|frame| frame.frame) else {
                return Err(Status::invalid_argument("List Operations: Expected Reply"));
            };
            let Some(child_response::Result::Payload(payload)) = child.result else {
                return Err(Status::invalid_argument("List Operations: Expected Inventory"));
            };
            let list = OperationsListResponse::decode(payload.as_slice())
                .map_err(|_| Status::invalid_argument("List Operations: Invalid Inventory"))?;
            let operation = list.operations.first().ok_or_else(|| Status::invalid_argument("List Operations: Missing Contract"))?;
            let id = operation.contract.as_ref().ok_or_else(|| Status::invalid_argument("List Operations: Missing Contract"))?.id.clone();
            send_child(output, 2, "harmos.plugin.v1.Operations/Get", OperationsGetRequest { id: id.clone() }.encode_to_vec()).await?;
            let Some(instance_invoke_request::Frame::Child(child)) = input.message().await?.and_then(|frame| frame.frame) else {
                return Err(Status::invalid_argument("Get Operation: Expected Reply"));
            };
            let Some(child_response::Result::Payload(payload)) = child.result else {
                return Err(Status::invalid_argument("Get Operation: Expected Contract"));
            };
            let get = OperationsGetResponse::decode(payload.as_slice())
                .map_err(|_| Status::invalid_argument("Get Operation: Invalid Contract"))?;
            if get.operation.as_ref() != Some(operation) {
                return Err(Status::invalid_argument("Get Operation: Contract Differs"));
            }
            response::Result::Payload(fixture::payload(id.as_bytes()))
        }
        "conformance.watch" => {
            output
                .send(Ok(InstanceInvokeResponse {
                    frame: Some(instance_invoke_response::Frame::Child(ChildRequest {
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
                    .send(Ok(InstanceInvokeResponse {
                        frame: Some(instance_invoke_response::Frame::ChildDemand(ChildDemand { call: 1 })),
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
                                .send(Ok(InstanceInvokeResponse {
                                    frame: Some(instance_invoke_response::Frame::ChildCancel(ChildCancel { call: 1 })),
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
                    send_child(output, call, "harmos.plugin.v1.Time/Get", Vec::new()).await?;
                    input.message().await?;
                }
                for (call, seconds, background) in [(3, 2, true), (4, 3600, false)] {
                    output
                        .send(Ok(InstanceInvokeResponse {
                            frame: Some(instance_invoke_response::Frame::Child(ChildRequest {
                                call,
                                operation: "harmos.plugin.v1.Time/Sleep".into(),
                                payload: TimeSleepRequest {
                                    duration: Some(Duration { seconds, nanos: 0 }),
                                }
                                .encode_to_vec(),
                                fingerprint: fixture::fingerprint("harmos.plugin.v1.Time/Sleep"),
                                background,
                                ..Default::default()
                            })),
                        }))
                        .await
                        .map_err(|_| Status::cancelled("Host Closed"))?;
                }
                input.message().await?;
                output
                    .send(Ok(InstanceInvokeResponse {
                        frame: Some(instance_invoke_response::Frame::ChildCancel(ChildCancel { call: 4 })),
                    }))
                    .await
                    .map_err(|_| Status::cancelled("Host Closed"))?;
                response::Result::Failure(Failure {
                    classification: Classification::TimedOut as i32,
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
                send_child(output, 1, "harmos.plugin.v1.Time/Get", Vec::new()).await?;
                input.message().await?;
                send_child(
                    output,
                    2,
                    "harmos.plugin.v1.Time/Sleep",
                    TimeSleepRequest {
                        duration: Some(Duration { seconds: 1, nanos: 0 }),
                    }
                    .encode_to_vec(),
                )
                .await?;
                input.message().await?;
                response::Result::Failure(Failure {
                    classification: Classification::TimedOut as i32,
                    code: "deadline".into(),
                    message: "Invoke Local: Overall Deadline Reached".into(),
                    ..Default::default()
                })
            } else if value == b"retry" {
                send_child(
                    output,
                    1,
                    "harmos.plugin.v1.Time/Sleep",
                    TimeSleepRequest {
                        duration: Some(Duration { seconds: 1, nanos: 11 }),
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
                (1, "harmos.plugin.v1.Time/Get", vec![]),
                (
                    2,
                    "harmos.plugin.v1.Time/Sleep",
                    TimeSleepRequest {
                        duration: Some(Duration { seconds: 3600, nanos: 0 }),
                    }
                    .encode_to_vec(),
                ),
                (3, "harmos.plugin.v1.Time/Get", vec![]),
            ] {
                send_child(output, call, operation, payload).await?;
                let Some(instance_invoke_request::Frame::Child(ChildResponse {
                    call: received,
                    result: Some(child_response::Result::Payload(payload)),
                })) = input.message().await?.and_then(|frame| frame.frame)
                else {
                    return Err(Status::invalid_argument("Expected Clock Reply"));
                };
                if received != call {
                    return Err(Status::invalid_argument("Unexpected Clock Reply"));
                }
                if operation == "harmos.plugin.v1.Time/Get" {
                    readings.push(
                        TimeGetResponse::decode(payload.as_slice())
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
            let child = ChildRequest {
                call: 1,
                operation: "conformance.host.echo".into(),
                payload: request.payload,
                fingerprint: fixture::fingerprint("conformance.host.echo"),
                policy: Some(OperationPolicy {
                    retries: Some(RetryPolicy {
                        total_attempts: 3,
                        initial: Some(Duration { seconds: 1, nanos: 0 }),
                        maximum: Some(Duration { seconds: 4, nanos: 0 }),
                        deterministic_jitter: true,
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            };
            output
                .send(Ok(InstanceInvokeResponse {
                    frame: Some(instance_invoke_response::Frame::Child(child)),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            let Some(instance_invoke_request::Frame::Child(ChildResponse {
                call: 1,
                result: Some(child_response::Result::Payload(payload)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Expected First Child Reply"));
            };
            send_child(output, 2, "conformance.host.echo", payload).await?;
            let Some(instance_invoke_request::Frame::Child(ChildResponse {
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
                    next: Some(graph),
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
                    response::Result::Failure(Failure {
                        classification: Classification::Refused as i32,
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
                Some(instance_invoke_request::Frame::Child(ChildResponse {
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
            let mut replies = BTreeMap::new();
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
            let Some(instance_invoke_request::Frame::Child(ChildResponse {
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
                .send(Ok(InstanceInvokeResponse {
                    frame: Some(instance_invoke_response::Frame::ChildCancel(ChildCancel { call: 1 })),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            response::Result::Failure(Failure {
                classification: Classification::Cancelled as i32,
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
        .send(Ok(InstanceInvokeResponse {
            frame: Some(instance_invoke_response::Frame::Response(plugin::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn send_child(
    output: &mpsc::Sender<result::Result<InstanceInvokeResponse, Status>>, call: u64, operation: &str, payload: Vec<u8>,
) -> result::Result<(), Status> {
    let child = ChildRequest {
        call,
        operation: operation.into(),
        payload,
        fingerprint: fixture::fingerprint(operation),
        policy: None,
        ..Default::default()
    };
    output
        .send(Ok(InstanceInvokeResponse {
            frame: Some(instance_invoke_response::Frame::Child(child)),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))
}

async fn samples(
    open: SamplesSelection, input: &mut Streaming<InstanceSamplesRequest>,
    output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>,
) -> result::Result<(), Status> {
    if open.stream == "conformance.device" {
        let samples = InstanceSamplesResponse {
            samples: vec![Sample {
                at: None,
                row: fixture::payload(b"device"),
                sequence: 1,
            }],
            frame: None,
        };
        output.send(Ok(samples)).await.map_err(|_| Status::cancelled("Host Closed"))?;
    }
    let mut next_call = 0_u64;
    loop {
        let frame = tokio::select! {
            _ = output.closed() => return Ok(()),
            frame = input.message() => frame?,
        };
        let Some(frame) = frame else {
            return Ok(());
        };
        let Some(instance_samples_request::Frame::Window(window)) = frame.frame else {
            return Err(Status::invalid_argument("Receive Samples: Expected a Window"));
        };
        if open.stream == "conformance.device" {
            return Err(Status::invalid_argument("Sample Window: a device stream takes no windows"));
        }
        let Some(start) = window.start else {
            return Err(Status::invalid_argument("Sample Window: start time required"));
        };
        if start.nanos < 0 || start.nanos >= 1_000_000_000 || !(-62_135_596_800..=253_402_300_799).contains(&start.seconds) {
            return Err(Status::invalid_argument("Sample Window: start time out of range"));
        }
        let positive = window.period.is_some_and(|period| {
            period.seconds >= 0 && (0..1_000_000_000).contains(&period.nanos) && (period.seconds > 0 || period.nanos > 0)
        });
        if !positive {
            return Err(Status::invalid_argument("Sample Window: a positive period is required"));
        }
        if window.count == 0 {
            return Err(Status::invalid_argument("Sample Window: a window holds at least one sample"));
        }
        // A Window Observer Answers the Whole Grant; One-Sample Observers Run Once per Sample, in Order.
        let values = if open.stream == fixture::WINDOW {
            (0..window.count).map(|index| (window.sequence + index).to_le_bytes().to_vec()).collect()
        } else {
            let mut values = Vec::new();
            for index in 0..window.count {
                let value = if fixture::WINDOW_STREAMS.contains(&open.stream.as_str()) {
                    match samples_children(&open, input, output, &mut next_call).await {
                        Ok(value) => value,
                        Err(error) if error.code() == tonic::Code::Cancelled => return Ok(()),
                        Err(error) => return Err(error),
                    }
                } else if open.stream == fixture::OVERSIZED {
                    vec![
                        0;
                        usize::try_from(window.sequence + index)
                            .map_err(|_| Status::resource_exhausted("Answer Window: Sample Too Large"))?
                    ]
                } else {
                    (window.sequence + index).to_le_bytes().to_vec()
                };
                values.push(value);
            }
            values
        };
        for part in parts(values, harmos_protocol::transfer::default_message_bytes().get())? {
            output.send(Ok(part)).await.map_err(|_| Status::cancelled("Host Closed"))?;
        }
    }
}

/// Split a Window's Values at Sample Boundaries Into Parts That Each Fit the Advertised Message Size.
fn parts(values: Vec<Vec<u8>>, message_bytes: usize) -> result::Result<Vec<InstanceSamplesResponse>, Status> {
    // Upper Bound of the Response, Batch Offset, Count, Last and Columns Framing Around the Columns' Values.
    const FRAMING: usize = 64;
    let budget = message_bytes
        .checked_sub(FRAMING)
        .ok_or_else(|| Status::resource_exhausted("Answer Window: Raise the message size above the batch framing"))?;
    let mut parts = Vec::new();
    let mut columns = fixture::Values::default();
    let mut size = 0;
    let mut offset = 0_u64;
    for value in values {
        let cost = 1 + prost::encoding::encoded_len_varint(value.len() as u64) + value.len();
        if cost > budget {
            return Err(Status::resource_exhausted("Answer Window: a conformance.model.v1.Value sample exceeds the message size"));
        }
        if size + cost > budget {
            let count = columns.value.len() as u64;
            parts.push(batch(offset, std::mem::take(&mut columns), false));
            offset += count;
            size = 0;
        }
        size += cost;
        columns.value.push(value);
    }
    parts.push(batch(offset, columns, true));
    Ok(parts)
}

fn batch(offset: u64, columns: fixture::Values, last: bool) -> InstanceSamplesResponse {
    InstanceSamplesResponse {
        samples: Vec::new(),
        frame: Some(instance_samples_response::Frame::Batch(SampleBatch {
            offset,
            count: columns.value.len() as u64,
            columns: columns.encode_to_vec(),
            last,
        })),
    }
}

async fn samples_child(
    output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>, call: u64, operation: &str, payload: Vec<u8>, watch: bool,
) -> result::Result<(), Status> {
    let child = ChildRequest {
        call,
        operation: operation.into(),
        payload,
        fingerprint: fixture::fingerprint(operation),
        watch,
        max_response_bytes: if watch { 4096 } else { 0 },
        ..Default::default()
    };
    samples_frame(output, instance_samples_response::Frame::Child(child)).await
}

async fn samples_frame(
    output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>, frame: instance_samples_response::Frame,
) -> result::Result<(), Status> {
    output
        .send(Ok(InstanceSamplesResponse { samples: Vec::new(), frame: Some(frame) }))
        .await
        .map_err(|_| Status::cancelled("Samples: Host Closed"))
}

async fn samples_reply(
    input: &mut Streaming<InstanceSamplesRequest>, output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>,
) -> result::Result<ChildResponse, Status> {
    let frame = tokio::select! {
        _ = output.closed() => return Err(Status::cancelled("Samples: Demand Closed")),
        frame = input.message() => frame?,
    };
    let Some(frame) = frame else {
        return Err(Status::cancelled("Samples: Demand Closed"));
    };
    let Some(instance_samples_request::Frame::Child(child)) = frame.frame else {
        return Err(Status::invalid_argument("Samples: Expected Window Child Reply"));
    };
    Ok(child)
}

async fn samples_children(
    open: &SamplesSelection, input: &mut Streaming<InstanceSamplesRequest>,
    output: &mpsc::Sender<result::Result<InstanceSamplesResponse, Status>>, next_call: &mut u64,
) -> result::Result<Vec<u8>, Status> {
    *next_call += 1;
    let first = *next_call;
    match open.stream.as_str() {
        "conformance.window_caller_refusal" => {
            samples_child(output, first, "application.host.v1.Invocations/Create", crate::callers::request(), false).await?;
            let reply = samples_reply(input, output).await?;
            let Some(child_response::Result::Failure(failure)) = reply.result else {
                return Err(Status::invalid_argument("Window Client Operation: Expected Caller Refusal"));
            };
            Ok(failure.encode_to_vec())
        }
        "conformance.window_children" => {
            *next_call += 1;
            let second = *next_call;
            samples_child(output, first, "conformance.host.echo", fixture::payload(&[1]), false).await?;
            samples_child(output, second, "conformance.host.echo", fixture::payload(&[2]), false).await?;
            let mut replies = BTreeMap::new();
            for _ in 0..2 {
                let reply = samples_reply(input, output).await?;
                if ![first, second].contains(&reply.call) || replies.contains_key(&reply.call) {
                    return Err(Status::invalid_argument("Samples: Unknown or Duplicate Child"));
                }
                let Some(child_response::Result::Payload(payload)) = reply.result else {
                    return Err(Status::invalid_argument("Samples: Expected Echo Payload"));
                };
                let value = fixture::Value::decode(payload.as_slice()).map_err(|_| Status::invalid_argument("Samples: Invalid Echo"))?;
                replies.insert(reply.call, value.value);
            }
            Ok(replies.into_values().flatten().collect())
        }
        "conformance.window_context" => {
            let mut value = Vec::new();
            for (operation, payload) in [
                ("conformance.host.echo", fixture::payload(open.scope.as_bytes())),
                ("harmos.plugin.v1.Time/Get", Vec::new()),
                ("harmos.plugin.v1.plugins.Settings/List", Vec::new()),
            ] {
                let call = *next_call;
                samples_child(output, call, operation, payload, false).await?;
                let reply = samples_reply(input, output).await?;
                if reply.call != call {
                    return Err(Status::invalid_argument("Samples: Unknown Child"));
                }
                let Some(child_response::Result::Payload(payload)) = reply.result else {
                    return Err(Status::invalid_argument("Samples: Expected Context Result"));
                };
                let length = u32::try_from(payload.len()).map_err(|_| Status::resource_exhausted("Samples: Context Result Too Large"))?;
                value.extend(length.to_le_bytes());
                value.extend(payload);
                *next_call += 1;
            }
            Ok(value)
        }
        "conformance.window_failure" => {
            samples_child(output, first, "conformance.host.fail", fixture::payload(&[]), false).await?;
            let reply = samples_reply(input, output).await?;
            if reply.call != first {
                return Err(Status::invalid_argument("Samples: Unknown Child"));
            }
            let Some(child_response::Result::Failure(failure)) = reply.result else {
                return Err(Status::invalid_argument("Samples: Expected Failure"));
            };
            Ok(failure.encode_to_vec())
        }
        "conformance.window_watch" => {
            samples_child(output, first, "conformance.host.watch", fixture::payload(open.scope.as_bytes()), true).await?;
            let mut value = Vec::new();
            loop {
                samples_frame(output, instance_samples_response::Frame::ChildDemand(ChildDemand { call: first })).await?;
                let reply = samples_reply(input, output).await?;
                if reply.call != first {
                    return Err(Status::invalid_argument("Samples: Unknown Watch"));
                }
                match reply.result {
                    Some(child_response::Result::Item(item)) => value.extend(
                        fixture::Value::decode(item.as_slice()).map_err(|_| Status::invalid_argument("Samples: Invalid Watch Item"))?.value,
                    ),
                    Some(child_response::Result::Payload(_)) => return Ok(value),
                    _ => return Err(Status::invalid_argument("Samples: Expected Watch Item or Completion")),
                }
            }
        }
        "conformance.window_cancel" => {
            samples_child(output, first, "conformance.host.pending", fixture::payload(&[]), false).await?;
            *next_call += 1;
            let echo = *next_call;
            samples_child(output, echo, "conformance.host.echo", fixture::payload(b"release"), false).await?;
            let reply = samples_reply(input, output).await?;
            if reply.call != echo {
                return Err(Status::invalid_argument("Samples: Unknown Release Child"));
            }
            let Some(child_response::Result::Payload(_)) = reply.result else {
                return Err(Status::invalid_argument("Samples: Expected Release Result"));
            };
            samples_frame(output, instance_samples_response::Frame::ChildCancel(ChildCancel { call: first })).await?;
            *next_call += 1;
            let pending = *next_call;
            samples_child(output, pending, "conformance.host.pending", fixture::payload(&[]), false).await?;
            samples_frame(output, instance_samples_response::Frame::ChildCancel(ChildCancel { call: pending })).await?;
            Ok(b"cancelled".to_vec())
        }
        _ => Err(Status::not_found("Samples: Unknown Window Fixture")),
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[tokio::test]
    async fn reference_plugin_passes_every_rule() -> result::Result<(), Box<dyn Error>> {
        // Bind the Reference Plugin Before Starting the Rule Journey.
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let (shutdown, stopped) = oneshot::channel();
        let reference = Reference {
            started_at: fixture::timestamp(100),
            shutdown: Arc::new(Mutex::new(Some(shutdown))),
        };
        let incoming = stream::unfold(listener, |listener| async {
            let accepted = listener.accept().await.map(|(socket, _)| socket);
            Some((accepted, listener))
        });
        let maximum = harmos_protocol::transfer::default_message_bytes().get();
        let service = InstanceServer::new(reference).max_decoding_message_size(maximum).max_encoding_message_size(maximum);
        let server = tokio::spawn(tonic::transport::Server::builder().add_service(service).serve_with_incoming_shutdown(incoming, async {
            let _ = stopped.await;
        }));

        // Exercise the Identical Rule Set Used for Every SDK Fixture.
        let cases = crate::suite::run(&format!("http://{address}"), true).await?;
        let failures: Vec<_> = cases.into_iter().filter_map(|case| case.error.map(|error| (case.rule, error))).collect();
        server.await??;
        assert!(failures.is_empty(), "{failures:?}");
        Ok(())
    }
}

#[cfg(test)]
mod wrapper_tests {
    use super::Reference;
    use harmos_protocol::plugin::instance_server::InstanceServer;
    use prost_types::Timestamp;
    use std::sync::Arc;
    use tokio::{
        net::TcpListener,
        sync::{Mutex, oneshot},
    };
    use tonic::transport::Server;

    #[tokio::test]
    async fn product_wrapper_audiences_are_enforced_on_invocations_and_windows() {
        // Serve the Reference Carrier Without Owning an External Process.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = oneshot::channel();
        let shutdown = Arc::new(Mutex::new(Some(stop)));
        let reference = Reference {
            started_at: Timestamp { seconds: 0, nanos: 0 },
            shutdown: shutdown.clone(),
        };
        let incoming = futures_util::stream::unfold(listener, |listener| async {
            let accepted = listener.accept().await.map(|(socket, _)| socket);
            Some((accepted, listener))
        });
        let serving =
            tokio::spawn(Server::builder().add_service(InstanceServer::new(reference)).serve_with_incoming_shutdown(incoming, async {
                let _ = stopped.await;
            }));

        // Exercise the Same Product Wrapper Audience Rule as Every SDK Carrier.
        let result = crate::callers::run(34, &address).await;

        // Release the Reference Endpoint After Both Invocation and Samples Proof.
        if let Some(stop) = shutdown.lock().await.take() {
            let _ = stop.send(());
        }
        serving.await.unwrap().unwrap();
        result.unwrap();
    }
}
