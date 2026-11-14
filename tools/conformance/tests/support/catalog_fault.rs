/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Forward Real Plugin Traffic While Omitting Required SDK Identity.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::error::Error;

// Third Party Imports
use futures_util::stream;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tonic::{Request, Response, Status, Streaming, transport::Channel};

// Local Imports
use harmos_protocol::plugin::{
    instance_client::InstanceClient,
    instance_server::{Instance, InstanceServer},
};

use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest,
    InstanceSamplesResponse, InstanceStopRequest, InstanceStopResponse,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct CatalogFault {
    pub address: String,
    stop: oneshot::Sender<()>,
    task: JoinHandle<Result<(), tonic::transport::Error>>,
}

struct Forward {
    client: InstanceClient<Channel>,
    fault: Fault,
}

/* === Enums === */

#[derive(Clone, Copy)]
pub enum Fault {
    Identity,
    Fingerprint,
    Options,
}

/* === Traits === */

/* === Implementations === */

impl CatalogFault {
    pub async fn start(address: &str, fault: Fault) -> Result<Self, Box<dyn Error>> {
        let maximum = harmos_protocol::transfer::default_message_bytes().get();
        let client = InstanceClient::connect(format!("http://{address}"))
            .await?
            .max_decoding_message_size(maximum)
            .max_encoding_message_size(maximum);
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?.to_string();
        let incoming = stream::unfold(listener, |listener| async {
            let socket = listener.accept().await.map(|(socket, _)| socket);
            Some((socket, listener))
        });
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    InstanceServer::new(Forward { client, fault }).max_decoding_message_size(maximum).max_encoding_message_size(maximum),
                )
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                })
                .await
        });
        Ok(Self { address, stop, task })
    }

    pub async fn close(self) -> Result<(), Box<dyn Error>> {
        let _ = self.stop.send(());
        self.task.await??;
        Ok(())
    }
}

#[tonic::async_trait]
impl Instance for Forward {
    async fn get(&self, request: Request<InstanceGetRequest>) -> Result<Response<InstanceGetResponse>, Status> {
        let mut response = self.client.clone().get(request).await?;
        let description = response.get_mut();
        match self.fault {
            Fault::Identity => description.sdk.clear(),
            Fault::Fingerprint => {
                let catalog = description.catalog.as_mut().expect("reference catalog");
                let operation =
                    catalog.operations.iter_mut().find(|operation| operation.id == "conformance.schema").expect("schema operation");
                operation.fingerprint[0] ^= 1;
                catalog.digest = harmos_protocol::schema::catalog_digest(catalog);
            }
            Fault::Options => {
                let catalog = description.catalog.as_mut().expect("reference catalog");
                let types = catalog.types.as_mut().expect("reference types");
                for file in &mut types.file {
                    for message in &mut file.message_type {
                        if message.name.as_deref() == Some("Graph") {
                            message.options = None;
                        }
                    }
                }
                catalog.digest = harmos_protocol::schema::catalog_digest(catalog);
            }
        }
        Ok(response)
    }

    type InvokeStream = Streaming<InstanceInvokeResponse>;

    async fn invoke(&self, request: Request<Streaming<InstanceInvokeRequest>>) -> Result<Response<Self::InvokeStream>, Status> {
        let mut client = self.client.clone();
        let frames = relay(request.into_inner());
        client.invoke(frames).await
    }

    type SamplesStream = Streaming<InstanceSamplesResponse>;

    async fn samples(&self, request: Request<Streaming<InstanceSamplesRequest>>) -> Result<Response<Self::SamplesStream>, Status> {
        let mut client = self.client.clone();
        let frames = relay(request.into_inner());
        client.samples(frames).await
    }

    async fn stop(&self, request: Request<InstanceStopRequest>) -> Result<Response<InstanceStopResponse>, Status> {
        self.client.clone().stop(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn relay<T: Send + 'static>(mut input: Streaming<T>) -> tonic::codegen::tokio_stream::wrappers::ReceiverStream<T> {
    let (send, receive) = mpsc::channel(8);
    tokio::spawn(async move {
        while let Ok(Some(frame)) = input.message().await {
            if send.send(frame).await.is_err() {
                break;
            }
        }
    });
    tonic::codegen::tokio_stream::wrappers::ReceiverStream::new(receive)
}

/* === Tests === */
