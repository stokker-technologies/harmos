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
use harmos_protocol::v1::{
    self as v1, child_response, plugin_invoke_request, plugin_invoke_response, plugin_samples_request,
    plugin_server::{Plugin, PluginServer},
    response,
};

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
impl Plugin for Reference {
    async fn get(&self, _: Request<v1::PluginGetRequest>) -> std::result::Result<Response<v1::PluginGetResponse>, Status> {
        Ok(Response::new(v1::PluginGetResponse {
            id: "conformance".into(),
            version: "1.0.0".into(),
            started_at: Some(self.started_at),
            catalog: Some(fixture::catalog()),
            revision: 1,
            sdk: "hand_written_tonic".into(),
            sdk_version: "1.0.0".into(),
        }))
    }

    type InvokeStream = Output<v1::PluginInvokeResponse>;

    async fn invoke(
        &self, request: Request<Streaming<v1::PluginInvokeRequest>>,
    ) -> std::result::Result<Response<Self::InvokeStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(plugin_invoke_request::Frame::Request(request)) = first else {
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

    type SamplesStream = Output<v1::PluginSamplesResponse>;

    async fn samples(
        &self, request: Request<Streaming<v1::PluginSamplesRequest>>,
    ) -> std::result::Result<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(plugin_samples_request::Frame::Open(open)) = first else {
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

    async fn stop(&self, _: Request<v1::PluginStopRequest>) -> std::result::Result<Response<v1::PluginStopResponse>, Status> {
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        Ok(Response::new(v1::PluginStopResponse {}))
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
        .add_service(PluginServer::new(reference))
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
    request: v1::Request, input: &mut Streaming<v1::PluginInvokeRequest>,
    output: &mpsc::Sender<std::result::Result<v1::PluginInvokeResponse, Status>>,
) -> std::result::Result<(), Status> {
    let result = match request.operation.as_str() {
        "conformance.clock" => {
            let mut readings = Vec::new();
            for (call, operation, payload) in [
                (1, "harmos.time.get", vec![]),
                (
                    2,
                    "harmos.time.sleep",
                    v1::TimeSleepRequest {
                        duration: Some(prost_types::Duration { seconds: 3600, nanos: 0 }),
                    }
                    .encode_to_vec(),
                ),
                (3, "harmos.time.get", vec![]),
            ] {
                send_child(output, call, operation, payload).await?;
                let Some(plugin_invoke_request::Frame::Child(v1::ChildResponse {
                    call: received,
                    result: Some(child_response::Result::Payload(payload)),
                })) = input.message().await?.and_then(|frame| frame.frame)
                else {
                    return Err(Status::invalid_argument("Expected Clock Reply"));
                };
                if received != call {
                    return Err(Status::invalid_argument("Unexpected Clock Reply"));
                }
                if operation == "harmos.time.get" {
                    readings.push(
                        v1::TimeGetResponse::decode(payload.as_slice())
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
            };
            output
                .send(Ok(v1::PluginInvokeResponse {
                    frame: Some(plugin_invoke_response::Frame::Child(child)),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            let Some(plugin_invoke_request::Frame::Child(v1::ChildResponse {
                call: 1,
                result: Some(child_response::Result::Payload(payload)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Expected First Child Reply"));
            };
            send_child(output, 2, "conformance.host.echo", payload).await?;
            let Some(plugin_invoke_request::Frame::Child(v1::ChildResponse {
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
        "conformance.echo" => {
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
        "conformance.private" => response::Result::Payload(request.payload),
        "conformance.resources" | "conformance.cancel_reason" => {
            send_child(output, 1, "conformance.host.echo", request.payload.clone()).await?;
            let frame = input.message().await?.and_then(|frame| frame.frame);
            if !matches!(
                frame,
                Some(plugin_invoke_request::Frame::Child(v1::ChildResponse {
                    call: 1,
                    result: Some(child_response::Result::Payload(_))
                }))
            ) {
                return Err(Status::invalid_argument("Expected Resource Child Reply"));
            }
            if request.operation == "conformance.resources" {
                response::Result::Payload(request.payload)
            } else {
                let Some(plugin_invoke_request::Frame::Cancel(cancel)) = input.message().await?.and_then(|frame| frame.frame) else {
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
                let Some(plugin_invoke_request::Frame::Child(child)) = frame else {
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
            let Some(plugin_invoke_request::Frame::Child(v1::ChildResponse {
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
            let Some(plugin_invoke_request::Frame::Cancel(cancel)) = frame else {
                return Err(Status::invalid_argument("Expected Cancel"));
            };
            output
                .send(Ok(v1::PluginInvokeResponse {
                    frame: Some(plugin_invoke_response::Frame::ChildCancel(v1::ChildCancel { call: 1 })),
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
        .send(Ok(v1::PluginInvokeResponse {
            frame: Some(plugin_invoke_response::Frame::Response(v1::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn send_child(
    output: &mpsc::Sender<std::result::Result<v1::PluginInvokeResponse, Status>>, call: u64, operation: &str, payload: Vec<u8>,
) -> std::result::Result<(), Status> {
    let child = v1::ChildRequest {
        call,
        operation: operation.into(),
        payload,
        fingerprint: fixture::fingerprint(operation),
        policy: None,
    };
    output
        .send(Ok(v1::PluginInvokeResponse {
            frame: Some(plugin_invoke_response::Frame::Child(child)),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))
}

async fn samples(
    open: v1::SamplesSelection, input: &mut Streaming<v1::PluginSamplesRequest>,
    output: &mpsc::Sender<std::result::Result<v1::PluginSamplesResponse, Status>>,
) -> std::result::Result<(), Status> {
    if open.stream == "conformance.device" {
        let samples = v1::PluginSamplesResponse {
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
        let Some(plugin_samples_request::Frame::Tick(tick)) = frame.frame else {
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
        let samples = v1::PluginSamplesResponse {
            samples: vec![v1::Sample { at: Some(at), row, sequence }],
        };
        output.send(Ok(samples)).await.map_err(|_| Status::cancelled("Host Closed"))?;
    }
}

/* === Tests === */
