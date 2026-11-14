/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Caller Controls One Generated Resource Call.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
};

// Third Party Imports
use futures_core::Stream;
use tokio::sync::oneshot;
use tonic::{Request, Response, Status};

// Local Imports
use super::{
    client::{JobsProgressWatchOptions, Root},
    host as app_host,
};
use harmos::client::CallOptions;
use harmos::{Client, ClientSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Endpoint(std::sync::Mutex<Option<oneshot::Sender<()>>>);
struct Watching {
    opening: Option<app_host::jobs::ProgressWatchResponse>,
    cancelled: Option<oneshot::Sender<()>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[tonic::async_trait]
impl app_host::jobs::progress_server::Progress for Endpoint {
    type WatchStream = Watching;
    async fn watch(&self, request: Request<app_host::jobs::ProgressWatchRequest>) -> Result<Response<Watching>, Status> {
        let metadata = request.metadata();
        let token = metadata.get("authorization").and_then(|value| value.to_str().ok());
        if token != Some("Bearer fixture") || metadata.get("grpc-timeout").is_none() {
            return Err(Status::unauthenticated("Watch Progress: metadata or deadline is missing"));
        }
        Ok(Response::new(Watching {
            opening: Some(app_host::jobs::ProgressWatchResponse { value: request.into_inner().value }),
            cancelled: self.0.lock().unwrap().take(),
        }))
    }
}
impl Stream for Watching {
    type Item = Result<app_host::jobs::ProgressWatchResponse, Status>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.opening.take() {
            Some(value) => Poll::Ready(Some(Ok(value))),
            None => Poll::Pending,
        }
    }
}
impl Drop for Watching {
    fn drop(&mut self) {
        if let Some(cancelled) = self.cancelled.take() {
            let _ = cancelled.send(());
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn one_call_preserves_metadata_deadline_override_and_cancellation() {
    // Serve the Generated Watch and Retain an Event for Transport Cancellation.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let incoming =
        futures_util::stream::unfold(listener, |listener| async { Some((listener.accept().await.map(|(stream, _)| stream), listener)) });
    let (cancel, cancelled) = oneshot::channel();
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(app_host::jobs::progress_server::ProgressServer::new(Endpoint(std::sync::Mutex::new(Some(cancel)))))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = stopped.await;
            }),
    );
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let maximum = harmos_protocol::transfer::default_message_bytes();
    let settings = ClientSettings::for_storage(Default::default(), maximum.checked_mul(NonZeroUsize::new(6).unwrap()).unwrap()).unwrap();
    let client = Root::new(Client::new(channel, settings).unwrap().transport_handle());
    // Bind Metadata and a Per-Call Deadline to the Generated Root Request.
    let mut metadata = tonic::metadata::MetadataMap::new();
    metadata.insert("authorization", "Bearer fixture".parse().unwrap());
    let options = JobsProgressWatchOptions {
        context: CallOptions {
            metadata,
            timeout: Some(std::time::Duration::from_secs(17)),
        },
    };
    let mut watching = client.jobs.progress.watch_with("selection", options).await.unwrap();
    assert_eq!(watching.next().await.unwrap().unwrap(), "selection");
    // Dropping This Call Cancels Its Transport and Releases the Server Stream.
    drop(watching);
    cancelled.await.unwrap();
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}
