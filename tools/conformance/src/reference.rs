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
use prost_types::{Duration, Timestamp};
use std::{collections::BTreeMap, result::Result as StdResult};
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
use crate::conformance::{Graph, graph::Nested};
use crate::{Result, fixture};
use harmos_protocol::plugin::{child_response, response};
use harmos_protocol::plugin::{
    instance_invoke_request, instance_invoke_response, instance_samples_request, instance_samples_response,
    instance_server::{Instance, InstanceServer},
};

use harmos_protocol::host::plugins::SettingsListResponse;
use harmos_protocol::host::{OperationsGetRequest, OperationsGetResponse, OperationsListResponse, TimeGetResponse, TimeSleepRequest};
use harmos_protocol::model::RetryPolicy;
use harmos_protocol::model::{Classification, Failure, OperationPolicy};
use harmos_protocol::plugin::{
    ChildCancel, ChildDemand, ChildRequest, ChildResponse, Request as InvocationRequest, Response as InvocationResponse, Sample,
    SamplesSelection,
};
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
    InstanceSamplesResponse, InstanceStopRequest, InstanceStopResponse,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = StdResult<T, Status>> + Send>>;

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
    async fn get(&self, _: Request<InstanceGetRequest>) -> StdResult<Response<InstanceGetResponse>, Status> {
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

    async fn invoke(&self, request: Request<Streaming<InstanceInvokeRequest>>) -> StdResult<Response<Self::InvokeStream>, Status> {
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

    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> StdResult<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(instance_samples_request::Frame::Open(open)) = first else {
            return Err(Status::invalid_argument("Open Must Be First"));
        };
        if !["conformance.ticks", "conformance.device", "conformance.settings"].contains(&open.stream.as_str())
            && !fixture::TICK_STREAMS.contains(&open.stream.as_str())
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

    async fn stop(&self, _: Request<InstanceStopRequest>) -> StdResult<Response<InstanceStopResponse>, Status> {
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
    request: InvocationRequest, input: &mut Streaming<InstanceInvokeRequest>,
    output: &mpsc::Sender<StdResult<InstanceInvokeResponse, Status>>,
) -> StdResult<(), Status> {
    let result = match request.operation.as_str() {
        "conformance.caller_refusal" => {
            send_child(output, 1, "harmos.host.v1.Invocations/Create", crate::callers::request()).await?;
            let Some(instance_invoke_request::Frame::Child(ChildResponse {
                call: 1,
                result: Some(child_response::Result::Failure(failure)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Invoke Client Operation: Expected Caller Refusal"));
            };
            response::Result::Payload(fixture::payload(&failure.encode_to_vec()))
        }
        "conformance.settings_read" => {
            send_child(output, 1, "harmos.host.v1.plugins.Settings/List", Vec::new()).await?;
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
            send_child(output, 1, "harmos.host.v1.Operations/List", Vec::new()).await?;
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
            send_child(output, 2, "harmos.host.v1.Operations/Get", OperationsGetRequest { id: id.clone() }.encode_to_vec()).await?;
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
                    send_child(output, call, "harmos.host.v1.Time/Get", Vec::new()).await?;
                    input.message().await?;
                }
                for (call, seconds, background) in [(3, 2, true), (4, 3600, false)] {
                    output
                        .send(Ok(InstanceInvokeResponse {
                            frame: Some(instance_invoke_response::Frame::Child(ChildRequest {
                                call,
                                operation: "harmos.host.v1.Time/Sleep".into(),
                                payload: TimeSleepRequest {
                                    duration: Some(Duration { seconds, nanos: 0 }),
                                }
                                .encode_to_vec(),
                                fingerprint: fixture::fingerprint("harmos.host.v1.Time/Sleep"),
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
                send_child(output, 1, "harmos.host.v1.Time/Get", Vec::new()).await?;
                input.message().await?;
                send_child(
                    output,
                    2,
                    "harmos.host.v1.Time/Sleep",
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
                    "harmos.host.v1.Time/Sleep",
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
                (1, "harmos.host.v1.Time/Get", vec![]),
                (
                    2,
                    "harmos.host.v1.Time/Sleep",
                    TimeSleepRequest {
                        duration: Some(Duration { seconds: 3600, nanos: 0 }),
                    }
                    .encode_to_vec(),
                ),
                (3, "harmos.host.v1.Time/Get", vec![]),
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
                if operation == "harmos.host.v1.Time/Get" {
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
            frame: Some(instance_invoke_response::Frame::Response(InvocationResponse { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn send_child(
    output: &mpsc::Sender<StdResult<InstanceInvokeResponse, Status>>, call: u64, operation: &str, payload: Vec<u8>,
) -> StdResult<(), Status> {
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
    output: &mpsc::Sender<StdResult<InstanceSamplesResponse, Status>>,
) -> StdResult<(), Status> {
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
    let mut sequence = 0_u64;
    let mut next_call = 0_u64;
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
        let row = if fixture::TICK_STREAMS.contains(&open.stream.as_str()) {
            match samples_children(&open, input, output, &mut next_call).await {
                Ok(row) => row,
                Err(error) if error.code() == tonic::Code::Cancelled => return Ok(()),
                Err(error) => return Err(error),
            }
        } else {
            fixture::payload(&sequence.to_le_bytes())
        };
        let samples = InstanceSamplesResponse {
            samples: vec![Sample { at: Some(at), row, sequence }],
            frame: None,
        };
        output.send(Ok(samples)).await.map_err(|_| Status::cancelled("Host Closed"))?;
    }
}

async fn samples_child(
    output: &mpsc::Sender<StdResult<InstanceSamplesResponse, Status>>, call: u64, operation: &str, payload: Vec<u8>, watch: bool,
) -> StdResult<(), Status> {
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
    output: &mpsc::Sender<StdResult<InstanceSamplesResponse, Status>>, frame: instance_samples_response::Frame,
) -> StdResult<(), Status> {
    output
        .send(Ok(InstanceSamplesResponse { samples: Vec::new(), frame: Some(frame) }))
        .await
        .map_err(|_| Status::cancelled("Samples: Host Closed"))
}

async fn samples_reply(
    input: &mut Streaming<InstanceSamplesRequest>, output: &mpsc::Sender<StdResult<InstanceSamplesResponse, Status>>,
) -> StdResult<ChildResponse, Status> {
    let frame = tokio::select! {
        _ = output.closed() => return Err(Status::cancelled("Samples: Demand Closed")),
        frame = input.message() => frame?,
    };
    let Some(frame) = frame else {
        return Err(Status::cancelled("Samples: Demand Closed"));
    };
    let Some(instance_samples_request::Frame::Child(child)) = frame.frame else {
        return Err(Status::invalid_argument("Samples: Expected Tick Child Reply"));
    };
    Ok(child)
}

async fn samples_children(
    open: &SamplesSelection, input: &mut Streaming<InstanceSamplesRequest>,
    output: &mpsc::Sender<StdResult<InstanceSamplesResponse, Status>>, next_call: &mut u64,
) -> StdResult<Vec<u8>, Status> {
    *next_call += 1;
    let first = *next_call;
    match open.stream.as_str() {
        "conformance.tick_caller_refusal" => {
            samples_child(output, first, "harmos.host.v1.Invocations/Create", crate::callers::request(), false).await?;
            let reply = samples_reply(input, output).await?;
            let Some(child_response::Result::Failure(failure)) = reply.result else {
                return Err(Status::invalid_argument("Tick Client Operation: Expected Caller Refusal"));
            };
            Ok(fixture::payload(&failure.encode_to_vec()))
        }
        "conformance.tick_children" => {
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
            Ok(fixture::payload(&replies.into_values().flatten().collect::<Vec<_>>()))
        }
        "conformance.tick_context" => {
            let mut value = Vec::new();
            for (operation, payload) in [
                ("conformance.host.echo", fixture::payload(open.scope.as_bytes())),
                ("harmos.host.v1.Time/Get", Vec::new()),
                ("harmos.host.v1.plugins.Settings/List", Vec::new()),
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
            Ok(fixture::payload(&value))
        }
        "conformance.tick_failure" => {
            samples_child(output, first, "conformance.host.fail", fixture::payload(&[]), false).await?;
            let reply = samples_reply(input, output).await?;
            if reply.call != first {
                return Err(Status::invalid_argument("Samples: Unknown Child"));
            }
            let Some(child_response::Result::Failure(failure)) = reply.result else {
                return Err(Status::invalid_argument("Samples: Expected Failure"));
            };
            Ok(fixture::payload(&failure.encode_to_vec()))
        }
        "conformance.tick_watch" => {
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
                    Some(child_response::Result::Payload(_)) => return Ok(fixture::payload(&value)),
                    _ => return Err(Status::invalid_argument("Samples: Expected Watch Item or Completion")),
                }
            }
        }
        "conformance.tick_cancel" => {
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
            Ok(fixture::payload(b"cancelled"))
        }
        _ => Err(Status::not_found("Samples: Unknown Tick Fixture")),
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[tokio::test]
    async fn reference_plugin_passes_every_rule() -> StdResult<(), Box<dyn Error>> {
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
