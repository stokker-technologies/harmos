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
    self as v1, child_response, host_frame, plugin_frame,
    plugin_server::{Plugin, PluginServer},
    publish_frame, response,
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
    async fn describe(&self, _: Request<v1::DescribeRequest>) -> std::result::Result<Response<v1::Description>, Status> {
        Ok(Response::new(v1::Description {
            id: "conformance".into(),
            version: "1.0.0".into(),
            started_at: Some(self.started_at),
            catalog: Some(fixture::catalog()),
            revision: 1,
            sdk: "hand_written_tonic".into(),
            sdk_version: "1.0.0".into(),
        }))
    }

    type InvokeStream = Output<v1::PluginFrame>;

    async fn invoke(&self, request: Request<Streaming<v1::HostFrame>>) -> std::result::Result<Response<Self::InvokeStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(host_frame::Frame::Request(request)) = first else {
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

    type PublishStream = Output<v1::Samples>;

    async fn publish(&self, request: Request<Streaming<v1::PublishFrame>>) -> std::result::Result<Response<Self::PublishStream>, Status> {
        let mut input = request.into_inner();
        let first = input.message().await?.and_then(|frame| frame.frame);
        let Some(publish_frame::Frame::Open(open)) = first else {
            return Err(Status::invalid_argument("Open Must Be First"));
        };
        if !["conformance.ticks", "conformance.device", "conformance.settings"].contains(&open.stream.as_str()) {
            return Err(Status::not_found("Unknown Stream"));
        }
        let (output, receiver) = mpsc::channel(8);
        tokio::spawn(async move {
            let result = publish(open, &mut input, &output).await;
            if let Err(error) = result {
                let _ = output.send(Err(error)).await;
            }
        });
        Ok(Response::new(Box::pin(stream::unfold(
            receiver,
            |mut receiver| async move { receiver.recv().await.map(|item| (item, receiver)) },
        ))))
    }

    async fn stop(&self, _: Request<v1::StopRequest>) -> std::result::Result<Response<v1::StopResponse>, Status> {
        if let Some(shutdown) = self.shutdown.lock().await.take() {
            let _ = shutdown.send(());
        }
        Ok(Response::new(v1::StopResponse {}))
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
    request: v1::Request, input: &mut Streaming<v1::HostFrame>, output: &mpsc::Sender<std::result::Result<v1::PluginFrame, Status>>,
) -> std::result::Result<(), Status> {
    let result = match request.operation.as_str() {
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
                policy: Some(v1::OperationPolicy { attempts: 3, ..Default::default() }),
            };
            output
                .send(Ok(v1::PluginFrame {
                    frame: Some(plugin_frame::Frame::Child(child)),
                }))
                .await
                .map_err(|_| Status::cancelled("Host Closed"))?;
            let Some(host_frame::Frame::Child(v1::ChildResponse {
                call: 1,
                result: Some(child_response::Result::Payload(payload)),
            })) = input.message().await?.and_then(|frame| frame.frame)
            else {
                return Err(Status::invalid_argument("Expected First Child Reply"));
            };
            send_child(output, 2, "conformance.host.echo", payload).await?;
            let Some(host_frame::Frame::Child(v1::ChildResponse {
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
        "conformance.echo" => response::Result::Payload(request.payload),
        "conformance.failure" => response::Result::Failure(fixture::failure("conformance.failure")),
        "conformance.children" => {
            for call in [1, 2] {
                send_child(output, call, "conformance.host.echo", fixture::payload(&[call as u8])).await?;
            }
            let mut replies = std::collections::BTreeMap::new();
            while replies.len() < 2 {
                let frame = input.message().await?.and_then(|frame| frame.frame);
                let Some(host_frame::Frame::Child(child)) = frame else {
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
            let Some(host_frame::Frame::Child(v1::ChildResponse {
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
            let Some(host_frame::Frame::Cancel(cancel)) = frame else {
                return Err(Status::invalid_argument("Expected Cancel"));
            };
            output
                .send(Ok(v1::PluginFrame {
                    frame: Some(plugin_frame::Frame::ChildCancel(v1::ChildCancel { call: 1 })),
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
        .send(Ok(v1::PluginFrame {
            frame: Some(plugin_frame::Frame::Response(v1::Response { result: Some(result) })),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))?;
    Ok(())
}

async fn send_child(
    output: &mpsc::Sender<std::result::Result<v1::PluginFrame, Status>>, call: u64, operation: &str, payload: Vec<u8>,
) -> std::result::Result<(), Status> {
    let child = v1::ChildRequest {
        call,
        operation: operation.into(),
        payload,
        fingerprint: fixture::fingerprint(operation),
        policy: None,
    };
    output
        .send(Ok(v1::PluginFrame {
            frame: Some(plugin_frame::Frame::Child(child)),
        }))
        .await
        .map_err(|_| Status::cancelled("Host Closed"))
}

async fn publish(
    open: v1::PublishRequest, input: &mut Streaming<v1::PublishFrame>, output: &mpsc::Sender<std::result::Result<v1::Samples, Status>>,
) -> std::result::Result<(), Status> {
    if open.stream == "conformance.device" {
        let samples = v1::Samples {
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
        let Some(publish_frame::Frame::Tick(tick)) = frame.frame else {
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
        let samples = v1::Samples {
            samples: vec![v1::Sample { at: Some(at), row, sequence }],
        };
        output.send(Ok(samples)).await.map_err(|_| Status::cancelled("Host Closed"))?;
    }
}

/* === Tests === */
