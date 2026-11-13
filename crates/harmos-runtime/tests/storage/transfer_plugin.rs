/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Storage Child Calls Through the Plugin Carrier

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use futures_core::Stream;
use harmos_protocol::{plugin, schema, v1};
use prost::Message;
use std::{pin::Pin, sync::Arc};
use tonic::{Request, Response, Status, Streaming};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct AllowTransfers;

pub struct TransferPlugin {
    pub address: String,
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
struct Service {
    catalog: Arc<v1::Catalog>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::operations::authorization::Authorizer for AllowTransfers {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(Some(harmos_runtime::operations::authorization::Authorization::Restricted(
            harmos_runtime::operations::authorization::Required {
                contract: provider.contract.clone(),
                grants: ["unary-file", "abandoned-unary", "invalid-unary", "bounded-unary"]
                    .into_iter()
                    .map(|file| harmos_operations::plugin::Grant::of(provider.contract.operation().id.to_string(), file))
                    .collect(),
            },
        )))
    }
}

impl TransferPlugin {
    pub async fn start() -> Self {
        let types = harmos_protocol::storage_types().clone();
        let mut catalog = v1::Catalog {
            types: Some(types.clone()),
            ..Default::default()
        };
        for method in [
            "CreateFileProduction",
            "GetFileProduction",
            "AppendFileBytes",
            "CreateFileWriter",
            "PublishFileProduction",
            "DeleteFileProduction",
            "GetFileBytes",
            "ListRows",
            "Capabilities",
            "GetObject",
            "SetObject",
            "AppendRecords",
            "AppendSamples",
            "AppendObjectRevisions",
            "GetIndexDefinition",
            "SetIndexDefinition",
            "GetProjectionDefinition",
            "SetProjectionDefinition",
        ] {
            let request = format!("harmos.context.v1.Storage{method}Request");
            let response = format!("harmos.context.v1.Storage{method}Response");
            let method_id = method;
            let child = format!("harmos.context.v1.Storage/{method_id}");
            let id = format!("fixture.transfer.{method_id}");
            catalog.operations.push(v1::OperationContract {
                fingerprint: schema::fingerprint(&id, &types, &request, &response).unwrap(),
                id,
                request: request.clone(),
                response: response.clone(),
                visibility: v1::Visibility::Public as i32,
                ..Default::default()
            });
            catalog.requires.push(v1::Requirement {
                fingerprint: schema::fingerprint(&child, &types, &request, &response).unwrap(),
                id: child,
            });
        }
        catalog.digest = schema::catalog_digest(&catalog);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let incoming = futures_util::stream::unfold(listener, |listener| async move {
            let socket = listener.accept().await.map(|(socket, _)| socket);
            Some((socket, listener))
        });
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let service = Service { catalog: Arc::new(catalog) };
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(plugin::instance_server::InstanceServer::new(service))
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self { address, shutdown, task }
    }

    pub async fn stop(self) {
        let _ = self.shutdown.send(());
        self.task.await.unwrap();
    }
}

#[tonic::async_trait]
impl plugin::instance_server::Instance for Service {
    type InvokeStream = Pin<Box<dyn Stream<Item = Result<plugin::InstanceInvokeResponse, Status>> + Send>>;
    type SamplesStream = Pin<Box<dyn Stream<Item = Result<plugin::InstanceSamplesResponse, Status>> + Send>>;

    async fn get(&self, _: Request<plugin::InstanceGetRequest>) -> Result<Response<plugin::InstanceGetResponse>, Status> {
        Ok(Response::new(plugin::InstanceGetResponse {
            id: "transfer_fixture".into(),
            version: "1.0.0".into(),
            revision: 1,
            sdk: "fixture".into(),
            sdk_version: "1.0.0".into(),
            started_at: Some(prost_types::Timestamp { seconds: 1, nanos: 0 }),
            catalog: Some((*self.catalog).clone()),
        }))
    }

    async fn invoke(&self, request: Request<Streaming<plugin::InstanceInvokeRequest>>) -> Result<Response<Self::InvokeStream>, Status> {
        let mut input = request.into_inner();
        let Some(plugin::InstanceInvokeRequest {
            frame: Some(plugin::instance_invoke_request::Frame::Request(request)),
        }) = input.message().await?
        else {
            return Err(Status::invalid_argument("Invoke requires an opening request"));
        };
        let method = request.operation.strip_prefix("fixture.transfer.").ok_or_else(|| Status::not_found("Invoke unknown transfer"))?;
        let operation = format!("harmos.context.v1.Storage/{method}");
        let required = self
            .catalog
            .requires
            .iter()
            .find(|required| required.id == operation)
            .ok_or_else(|| Status::not_found("Invoke undeclared transfer"))?;
        let child = v1::ChildRequest {
            call: 1,
            operation,
            payload: request.payload,
            fingerprint: required.fingerprint.clone(),
            policy: None,
        };
        let (send, receive) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            if send
                .send(Ok(plugin::InstanceInvokeResponse {
                    frame: Some(plugin::instance_invoke_response::Frame::Child(child)),
                }))
                .await
                .is_err()
            {
                return;
            }
            while let Ok(Some(frame)) = input.message().await {
                if let Some(plugin::instance_invoke_request::Frame::Child(child)) = frame.frame {
                    let result = child.result.map(|result| match result {
                        v1::child_response::Result::Payload(payload) => v1::response::Result::Payload(payload),
                        v1::child_response::Result::Failure(failure) => v1::response::Result::Failure(failure),
                    });
                    let _ = send
                        .send(Ok(plugin::InstanceInvokeResponse {
                            frame: Some(plugin::instance_invoke_response::Frame::Response(v1::Response { result })),
                        }))
                        .await;
                    return;
                }
            }
        });
        let stream = futures_util::stream::unfold(receive, |mut receive| async move { receive.recv().await.map(|item| (item, receive)) });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn samples(&self, _: Request<Streaming<plugin::InstanceSamplesRequest>>) -> Result<Response<Self::SamplesStream>, Status> {
        Err(Status::not_found("Samples not declared"))
    }
    async fn stop(&self, _: Request<plugin::InstanceStopRequest>) -> Result<Response<plugin::InstanceStopResponse>, Status> {
        Ok(Response::new(plugin::InstanceStopResponse {}))
    }
}

/* === Public Functions === */

pub async fn call<O: harmos_operations::execution::operations::ProtobufOperation<Endpoints = ()>>(
    operations: &harmos_runtime::operations::OperationManager, request: O::Request,
) -> Result<O::Response, harmos_runtime::Error> {
    let method = O::ID.rsplit('/').next().unwrap();
    let bytes = operations.call_encoded(&format!("fixture.transfer.{method}"), request.encode_to_vec()).await?;
    O::Response::decode(bytes.as_slice()).map_err(|source| harmos_runtime::Error::OperationFailed {
        operation: O::ID.into(),
        source: Arc::new(source),
    })
}

pub async fn client_call<O: harmos_operations::execution::operations::ProtobufOperation<Endpoints = ()>>(
    channel: &tonic::transport::Channel, request: O::Request,
) -> Result<O::Response, tonic::Status> {
    let binding = O::GRPC.expect("client declaration");
    let path = tonic::codegen::http::uri::PathAndQuery::try_from(binding.path()).unwrap();
    let mut client = tonic::client::Grpc::new(channel.clone());
    client.ready().await.map_err(|source| tonic::Status::unavailable(source.to_string()))?;
    let response = client.unary(tonic::Request::new(request), path, tonic_prost::ProstCodec::default()).await?;
    Ok(response.into_inner())
}

/* === Private Functions === */

/* === Tests === */
