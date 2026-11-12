/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Resource Roots Compose Library and Product Services on One Channel.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

use super::{composed_resources::Root, v1};
use harmos::client::{Client, ClientSettings};
use std::{num::NonZeroUsize, pin::Pin};
use tonic::{Request, Response, Status};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Endpoint;
/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[tonic::async_trait]
impl v1::plugins_server::Plugins for Endpoint {
    async fn get(&self, request: Request<v1::PluginsGetRequest>) -> Result<Response<v1::PluginsGetResponse>, Status> {
        Ok(Response::new(v1::PluginsGetResponse {
            value: format!("plugins:{}", request.into_inner().value),
        }))
    }
}
#[tonic::async_trait]
impl v1::plugins::settings_server::Settings for Endpoint {
    async fn get(&self, request: Request<v1::plugins::SettingsGetRequest>) -> Result<Response<v1::plugins::SettingsGetResponse>, Status> {
        Ok(Response::new(v1::plugins::SettingsGetResponse {
            value: format!("settings:{}", request.into_inner().value),
        }))
    }
}
#[tonic::async_trait]
impl v1::jobs_server::Jobs for Endpoint {
    async fn start(&self, request: Request<v1::JobsStartRequest>) -> Result<Response<v1::JobsStartResponse>, Status> {
        Ok(Response::new(v1::JobsStartResponse {
            value: format!("start:{}", request.into_inner().value),
        }))
    }
}
#[tonic::async_trait]
impl v1::jobs::progress_server::Progress for Endpoint {
    type WatchStream = Pin<Box<dyn futures_core::Stream<Item = Result<v1::jobs::ProgressWatchResponse, Status>> + Send>>;
    async fn watch(&self, request: Request<v1::jobs::ProgressWatchRequest>) -> Result<Response<Self::WatchStream>, Status> {
        let value = request.into_inner().value;
        Ok(Response::new(Box::pin(futures_util::stream::iter([
            Ok(v1::jobs::ProgressWatchResponse { value: format!("opened:{value}") }),
            Ok(v1::jobs::ProgressWatchResponse { value: format!("finished:{value}") }),
        ]))))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn library_and_product_roots_share_parents_and_preserve_unary_and_watch_calls() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let incoming =
        futures_util::stream::unfold(listener, |listener| async { Some((listener.accept().await.map(|(stream, _)| stream), listener)) });
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let serving = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(v1::plugins_server::PluginsServer::new(Endpoint))
            .add_service(v1::plugins::settings_server::SettingsServer::new(Endpoint))
            .add_service(v1::jobs_server::JobsServer::new(Endpoint))
            .add_service(v1::jobs::progress_server::ProgressServer::new(Endpoint))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = stopped.await;
            }),
    );
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let settings = ClientSettings {
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        message_bytes: NonZeroUsize::new(4096).unwrap(),
        input_items: NonZeroUsize::MIN,
        input_bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let client = Root::new(Client::new(channel, settings).unwrap().transport_handle());
    assert_eq!(
        client.plugins.get(Request::new(v1::PluginsGetRequest { value: "selection".into() })).await.unwrap().into_inner().value,
        "plugins:selection"
    );
    assert_eq!(
        client
            .plugins
            .settings
            .get(Request::new(v1::plugins::SettingsGetRequest { value: "selection".into() }))
            .await
            .unwrap()
            .into_inner()
            .value,
        "settings:selection"
    );
    assert_eq!(
        client.jobs.start(Request::new(v1::JobsStartRequest { value: "selection".into() })).await.unwrap().into_inner().value,
        "start:selection"
    );
    let mut progress =
        client.jobs.progress.watch(Request::new(v1::jobs::ProgressWatchRequest { value: "selection".into() })).await.unwrap();
    assert_eq!(progress.next().await.unwrap().unwrap().value, "opened:selection");
    assert_eq!(progress.next().await.unwrap().unwrap().value, "finished:selection");
    assert!(progress.next().await.unwrap().is_none());
    stop.send(()).unwrap();
    serving.await.unwrap().unwrap();
}
