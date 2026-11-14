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

use super::client::PluginsSettingsSetOptions;
use super::{
    client::{AnnotationsGetResponse, JobsStartResponse, ProgressWatchResponse, Root},
    v1,
};
use futures_core::Stream;
use harmos::{Client, ClientSettings, SettingValues};
use harmos_protocol::{
    host::{self, status},
    model::StorageTransferSettings,
};
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
impl host::status_server::Status for Endpoint {
    async fn get(&self, _: Request<host::StatusGetRequest>) -> Result<Response<host::StatusGetResponse>, Status> {
        Ok(Response::new(host::StatusGetResponse {
            status: Some(host::HostStatus {
                identity: Some(host::HostIdentity {
                    application_name: "status:selection".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }))
    }
    type WatchStream = Pin<Box<dyn Stream<Item = Result<host::StatusWatchResponse, Status>> + Send>>;
    async fn watch(&self, _: Request<host::StatusWatchRequest>) -> Result<Response<Self::WatchStream>, Status> {
        Ok(Response::new(Box::pin(futures_util::stream::empty())))
    }
}
#[tonic::async_trait]
impl status::diagnostics_server::Diagnostics for Endpoint {
    async fn get(&self, request: Request<status::DiagnosticsGetRequest>) -> Result<Response<status::DiagnosticsGetResponse>, Status> {
        Ok(Response::new(status::DiagnosticsGetResponse {
            name: format!("diagnostics:{}", request.into_inner().name),
            ..Default::default()
        }))
    }
}
#[tonic::async_trait]
impl v1::jobs_server::Jobs for Endpoint {
    async fn start(&self, request: Request<v1::JobsStartRequest>) -> Result<Response<JobsStartResponse>, Status> {
        Ok(Response::new(JobsStartResponse {
            value: format!("start:{}", request.into_inner().value),
        }))
    }
}
#[tonic::async_trait]
impl v1::plugins::annotations_server::Annotations for Endpoint {
    async fn get(&self, request: Request<v1::plugins::AnnotationsGetRequest>) -> Result<Response<AnnotationsGetResponse>, Status> {
        Ok(Response::new(AnnotationsGetResponse { value: request.into_inner().name }))
    }
}
#[tonic::async_trait]
impl v1::jobs::progress_server::Progress for Endpoint {
    type WatchStream = Pin<Box<dyn Stream<Item = Result<ProgressWatchResponse, Status>> + Send>>;
    async fn watch(&self, request: Request<v1::jobs::ProgressWatchRequest>) -> Result<Response<Self::WatchStream>, Status> {
        let value = request.into_inner().value;
        Ok(Response::new(Box::pin(futures_util::stream::iter([
            Ok(ProgressWatchResponse { value: format!("opened:{value}") }),
            Ok(ProgressWatchResponse { value: format!("finished:{value}") }),
        ]))))
    }
}

#[tonic::async_trait]
impl host::plugins::settings_server::Settings for Endpoint {
    async fn list(&self, _: Request<host::plugins::SettingsListRequest>) -> Result<Response<host::plugins::SettingsListResponse>, Status> {
        Ok(Response::new(host::plugins::SettingsListResponse::default()))
    }
    async fn set(
        &self, request: Request<host::plugins::SettingsSetRequest>,
    ) -> Result<Response<host::plugins::SettingsSetResponse>, Status> {
        Ok(Response::new(host::plugins::SettingsSetResponse {
            revision: request.into_inner().request_id.into_bytes(),
            ..Default::default()
        }))
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
            .add_service(host::status_server::StatusServer::new(Endpoint))
            .add_service(status::diagnostics_server::DiagnosticsServer::new(Endpoint))
            .add_service(v1::jobs_server::JobsServer::new(Endpoint))
            .add_service(host::plugins::settings_server::SettingsServer::new(Endpoint))
            .add_service(v1::jobs::progress_server::ProgressServer::new(Endpoint))
            .add_service(v1::plugins::annotations_server::AnnotationsServer::new(Endpoint))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = stopped.await;
            }),
    );
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}")).unwrap().connect().await.unwrap();
    let settings = ClientSettings {
        transfer: StorageTransferSettings { chunk_bytes: Some(1024) },
        retained_bytes: NonZeroUsize::new(4096).unwrap(),
        message_bytes: NonZeroUsize::new(4096).unwrap(),
        input_items: NonZeroUsize::MIN,
        input_bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let incompatible = ClientSettings {
        message_bytes: NonZeroUsize::MIN,
        ..settings
    };
    assert!(matches!(Client::new(channel.clone(), incompatible), Err(error) if error.code() == tonic::Code::InvalidArgument));
    let client = Root::new(Client::new(channel, settings).unwrap().transport_handle());
    assert_eq!(client.status.get().await.unwrap().status.unwrap().identity.unwrap().application_name, "status:selection");
    assert_eq!(client.status.diagnostics.get("selection", 0, 512).await.unwrap().name, "diagnostics:selection");
    assert_eq!(client.jobs.start("selection").await.unwrap(), "start:selection");
    let mut progress = client.jobs.progress.watch("selection").await.unwrap();
    assert_eq!(progress.next().await.unwrap().unwrap(), "opened:selection");
    assert_eq!(progress.next().await.unwrap().unwrap(), "finished:selection");
    assert!(progress.next().await.unwrap().is_none());
    assert_eq!(client.plugins.annotations.get("example").await.unwrap(), "example");
    let generated = client.plugins.settings.set("example", SettingValues::default(), Vec::new()).await.unwrap();
    assert!(!generated.revision.is_empty());
    let replayed = client
        .plugins
        .settings
        .set_with(
            "example",
            SettingValues::default(),
            Vec::new(),
            PluginsSettingsSetOptions {
                request_id: Some("replay".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(replayed.revision, b"replay");
    stop.send(()).unwrap();
    serving.await.unwrap().unwrap();
}
