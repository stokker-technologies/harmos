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
    future::Future,
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
};

// Third Party Imports
use futures_core::Stream;
use prost_reflect::MessageDescriptor;
use tokio::sync::oneshot;
use tonic::codegen::http::uri::PathAndQuery;
use tonic::{Request, Response, Status};

// Local Imports
use super::{
    client::{JobsProgressWatchOptions, Root},
    host as app_host,
};
use harmos::client::{CallOptions, ClientTransport, Input, Method, NoItems, Running, Watching as ClientWatching};
use harmos::plugin::{Context as PluginContext, Watching as PluginWatching};
use harmos::{Client, ClientSettings};
use harmos_protocol::host::InvocationsWatchRequest;

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
    let context = CallOptions {
        metadata,
        timeout: Some(std::time::Duration::from_secs(17)),
    };
    let copied = context.clone();
    assert_eq!(copied.metadata.get("authorization"), context.metadata.get("authorization"));
    assert_eq!(copied.timeout, context.timeout);
    let _diagnostic = format!("{copied:?}");
    let options = JobsProgressWatchOptions { context: copied };
    let mut watching = client.jobs.progress.watch_with("selection", options).await.unwrap();
    assert_eq!(watching.next().await.unwrap().unwrap(), "selection");
    // Dropping This Call Cancels Its Transport and Releases the Server Stream.
    drop(watching);
    cancelled.await.unwrap();
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[test]
fn public_client_futures_are_send() {
    fn assert_send(_: impl Future + Send) {}

    // Type-Check Futures Without Connecting or Polling Them.
    let _ = |client: Client, unary: Method<(), ()>, managed: Method<(), (), (), ()>, watch: Method<(), (), NoItems, ()>| {
        assert_send(client.call(&unary, ()));
        assert_send(client.call_with(&unary, (), CallOptions::default()));
        assert_send(client.start(&managed, ()));
        assert_send(client.start_with(&managed, (), CallOptions::default()));
        assert_send(client.watch(&watch, ()));
        assert_send(client.watch_with(&watch, (), CallOptions::default()));
    };
    let _ = |mut call: Running<(), (), ()>| {
        assert_send(call.send(()));
        assert_send(call.finish_input());
        assert_send(call.next());
        assert_send(call.request_cancel());
        assert_send(call.join());
    };
    let _ = |call: Running<(), (), ()>| assert_send(call.cancel());
    let _ = |mut input: Input<()>| {
        assert_send(input.send(()));
        assert_send(input.finish());
    };
    let _ = |mut watch: ClientWatching<()>| assert_send(watch.next());
    let _ = |transport: ClientTransport, response: MessageDescriptor| {
        assert_send(transport.unary::<(), ()>(PathAndQuery::from_static("/fixture/Call"), Request::new(()), response.clone()));
        assert_send(transport.watch_known::<(), ()>(PathAndQuery::from_static("/fixture/Watch"), Request::new(()), response));
    };
    let _ = |root: Root| {
        assert_send(root.status.get());
        assert_send(root.jobs.start("selection"));
    };
    let _ = |context: PluginContext| {
        assert_send(context.cancelled());
        assert_send(context.watch(InvocationsWatchRequest::default(), NonZeroUsize::MIN));
    };
    let _ = |mut watch: PluginWatching<()>| assert_send(watch.next());
    let maximum = harmos_protocol::transfer::default_message_bytes();
    let settings = ClientSettings::for_storage(Default::default(), maximum.checked_mul(NonZeroUsize::new(6).unwrap()).unwrap()).unwrap();
    assert_send(Client::connect("127.0.0.1:1", settings));
}
