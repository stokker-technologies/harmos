/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Batched Samples Wire Carrier: A Separate Plugin Process Answers Each Tick With a Window

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::plugin::InstanceGetRequest;
use harmos_protocol::plugin::InstanceGetResponse;
use harmos_protocol::plugin::InstanceInvokeRequest;
use harmos_protocol::plugin::InstanceInvokeResponse;
use harmos_protocol::plugin::InstanceSamplesRequest;
use harmos_protocol::plugin::InstanceSamplesResponse;
use harmos_protocol::plugin::InstanceStopRequest;
use harmos_protocol::plugin::InstanceStopResponse;
use harmos_protocol::plugin::Sample;
use harmos_protocol::plugin::SamplesSelection;
use harmos_protocol::plugin::Tick;
use harmos_protocol::plugin::instance_client::InstanceClient;
use harmos_protocol::plugin::instance_samples_request::Frame;
use harmos_protocol::plugin::instance_server::{Instance, InstanceServer};

// Standard Library Imports
use std::{pin::Pin, process::Stdio, time::Duration};

// Third Party Imports
use futures_util::{Stream, StreamExt, stream};
use prost::Message as _;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};
use tonic::{Request, Response, Status, Streaming};

// Local Imports

/* === Constants === */

pub const MODE: &str = "--samples-carrier";

/* === Macros === */

/* === Type Aliases === */

type Output<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

/* === Structs === */

#[derive(Clone, PartialEq, prost::Message)]
struct Reading {
    #[prost(double, tag = "1")]
    value: f64,
}

#[derive(Clone, PartialEq, prost::Message)]
struct Window {
    #[prost(double, repeated, packed = "true", tag = "1")]
    values: Vec<f64>,
}

struct Carrier;

pub struct Connection {
    child: tokio::process::Child,
    output: mpsc::Sender<InstanceSamplesRequest>,
    input: Streaming<InstanceSamplesResponse>,
    packed: bool,
    values: usize,
    sequence: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[tonic::async_trait]
impl Instance for Carrier {
    async fn get(&self, _: Request<InstanceGetRequest>) -> Result<Response<InstanceGetResponse>, Status> {
        Err(Status::unimplemented("Samples Carrier"))
    }

    type InvokeStream = Output<InstanceInvokeResponse>;
    async fn invoke(&self, _: Request<Streaming<InstanceInvokeRequest>>) -> Result<Response<Self::InvokeStream>, Status> {
        Err(Status::unimplemented("Samples Carrier"))
    }

    type SamplesStream = Output<InstanceSamplesResponse>;
    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> Result<Response<Self::SamplesStream>, Status> {
        let mut input = request.into_inner();
        let Some(Ok(InstanceSamplesRequest { frame: Some(Frame::Open(selection)) })) = input.next().await else {
            return Err(Status::invalid_argument("Samples Carrier: Open First"));
        };
        let (shape, values) = selection.stream.split_once(':').unwrap();
        let packed = shape == "packed";
        let values: usize = values.parse().unwrap();
        let mut sequence = 0_u64;
        let answers = input.filter_map(move |frame| {
            let answer = match frame {
                Ok(InstanceSamplesRequest { frame: Some(Frame::Tick(tick)) }) => {
                    // The Plugin Samples Locally and Returns the Whole Window as One Message.
                    let samples = if packed {
                        sequence += 1;
                        let window = Window {
                            values: (0..values).map(|offset| (sequence as usize * values + offset) as f64).collect(),
                        };
                        vec![Sample {
                            at: tick.at,
                            row: window.encode_to_vec(),
                            sequence,
                        }]
                    } else {
                        (0..values)
                            .map(|_| {
                                sequence += 1;
                                Sample {
                                    at: tick.at,
                                    row: Reading { value: sequence as f64 }.encode_to_vec(),
                                    sequence,
                                }
                            })
                            .collect()
                    };
                    Some(Ok(InstanceSamplesResponse { samples, frame: None }))
                }
                Ok(_) => Some(Err(Status::invalid_argument("Samples Carrier: Ticks Only"))),
                Err(_) => None,
            };
            std::future::ready(answer)
        });
        Ok(Response::new(Box::pin(answers)))
    }

    async fn stop(&self, _: Request<InstanceStopRequest>) -> Result<Response<InstanceStopResponse>, Status> {
        Ok(Response::new(InstanceStopResponse {}))
    }
}

impl Connection {
    /// Start the Carrier Process and Open One Samples Call Shaped `repeated:<n>` or `packed:<n>`.
    pub async fn open(shape: &str) -> Self {
        let executable = std::env::current_exe().unwrap();
        let mut command = tokio::process::Command::new(executable);
        command.arg(MODE).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true);
        let mut child = command.spawn().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let address = tokio::time::timeout(Duration::from_secs(30), lines.next_line()).await.unwrap().unwrap().unwrap();
        let limit = harmos_protocol::transfer::default_message_bytes().get();
        let mut client = InstanceClient::connect(format!("http://{address}"))
            .await
            .unwrap()
            .max_decoding_message_size(limit)
            .max_encoding_message_size(limit);
        let (output, requests) = mpsc::channel(1);
        output
            .send(InstanceSamplesRequest {
                frame: Some(Frame::Open(SamplesSelection {
                    stream: shape.into(),
                    scope: String::new(),
                })),
            })
            .await
            .unwrap();
        let requests = stream::unfold(requests, |mut requests| async { requests.recv().await.map(|frame| (frame, requests)) });
        let input = client.samples(requests).await.unwrap().into_inner();
        let (kind, values) = shape.split_once(':').unwrap();
        Self {
            child,
            output,
            input,
            packed: kind == "packed",
            values: values.parse().unwrap(),
            sequence: 0,
        }
    }

    /// One Tick Round Trip: Send the Tick, Receive One Window, Decode Every Value.
    pub async fn tick(&mut self, at: prost_types::Timestamp) -> f64 {
        self.output
            .send(InstanceSamplesRequest {
                frame: Some(Frame::Tick(Tick { at: Some(at) })),
            })
            .await
            .unwrap();
        let response = self.input.message().await.unwrap().unwrap();
        let mut total = 0.0;
        if self.packed {
            assert_eq!(response.samples.len(), 1);
            let window = Window::decode(response.samples[0].row.as_slice()).unwrap();
            assert_eq!(window.values.len(), self.values);
            total += window.values.iter().sum::<f64>();
            self.sequence += 1;
        } else {
            assert_eq!(response.samples.len(), self.values);
            for sample in &response.samples {
                self.sequence += 1;
                assert_eq!(sample.sequence, self.sequence);
                total += Reading::decode(sample.row.as_slice()).unwrap().value;
            }
        }
        total
    }

    pub async fn close(mut self) {
        drop(self.output);
        drop(self.input);
        drop(self.child.stdin.take());
        let _ = tokio::time::timeout(Duration::from_secs(30), self.child.wait()).await;
    }
}

/* === Public Functions === */

/// Serve the Carrier Until the Host Closes Standard Input.
pub fn serve() {
    // Standard Input Closing Is the Host's Stop Signal.
    std::thread::spawn(|| {
        let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut Vec::new());
        std::process::exit(0);
    });
    let executor = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    executor.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        println!("{}", listener.local_addr().unwrap());
        let incoming = stream::unfold(listener, |listener| async move {
            let socket = listener.accept().await.map(|(socket, _)| socket);
            Some((socket, listener))
        });
        let limit = harmos_protocol::transfer::default_message_bytes().get();
        tonic::transport::Server::builder()
            .add_service(InstanceServer::new(Carrier).max_decoding_message_size(limit).max_encoding_message_size(limit))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
}

/* === Private Functions === */

/* === Tests === */
