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

// Third Party Imports
use futures_util::stream;
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use tonic::{Request, Response, Status, Streaming, transport::Channel};

// Local Imports
use harmos_protocol::v1::{
    self as v1,
    plugin_client::PluginClient,
    plugin_server::{Plugin, PluginServer},
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
    client: PluginClient<Channel>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl CatalogFault {
    pub async fn start(address: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = PluginClient::connect(format!("http://{address}")).await?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?.to_string();
        let incoming = stream::unfold(listener, |listener| async {
            let socket = listener.accept().await.map(|(socket, _)| socket);
            Some((socket, listener))
        });
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(PluginServer::new(Forward { client }))
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                })
                .await
        });
        Ok(Self { address, stop, task })
    }

    pub async fn close(self) -> Result<(), Box<dyn std::error::Error>> {
        let _ = self.stop.send(());
        self.task.await??;
        Ok(())
    }
}

#[tonic::async_trait]
impl Plugin for Forward {
    async fn describe(&self, request: Request<v1::DescribeRequest>) -> Result<Response<v1::Description>, Status> {
        let mut response = self.client.clone().describe(request).await?;
        response.get_mut().sdk.clear();
        Ok(response)
    }

    type InvokeStream = Streaming<v1::PluginFrame>;

    async fn invoke(&self, request: Request<Streaming<v1::HostFrame>>) -> Result<Response<Self::InvokeStream>, Status> {
        let mut client = self.client.clone();
        let frames = relay(request.into_inner());
        client.invoke(frames).await
    }

    type PublishStream = Streaming<v1::Samples>;

    async fn publish(&self, request: Request<Streaming<v1::PublishFrame>>) -> Result<Response<Self::PublishStream>, Status> {
        let mut client = self.client.clone();
        let frames = relay(request.into_inner());
        client.publish(frames).await
    }

    async fn stop(&self, request: Request<v1::StopRequest>) -> Result<Response<v1::StopResponse>, Status> {
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
