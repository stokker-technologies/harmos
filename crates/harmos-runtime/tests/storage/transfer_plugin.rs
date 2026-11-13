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
    #[cfg(test)]
    next_policy: Arc<std::sync::Mutex<Option<v1::OperationPolicy>>>,
    #[cfg(test)]
    watch_opened: Arc<tokio::sync::Notify>,
    shutdown: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Clone)]
struct Service {
    watch_opened: Arc<tokio::sync::Notify>,
    catalog: Arc<v1::Catalog>,
    next_policy: Arc<std::sync::Mutex<Option<v1::OperationPolicy>>>,
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
            "GetFileHash",
            "CaptureFile",
            "ListDirectoryEntries",
            "WatchRows",
            "WatchFileBytes",
            "WatchFileProduction",
            "WatchDirectoryEntries",
            "ListColumnRows",
            "ListIndexEntries",
            "ListProjectionRows",
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
            let method_id = snake(method);
            let child = format!("harmos.context.v1.Storage/{method}");
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
                fingerprint: schema::host_fingerprint(
                    &child,
                    &v1::Protobuf {
                        descriptors: types.encode_to_vec(),
                        request: request.clone(),
                        response: response.clone(),
                        input: None,
                        output: method.starts_with("Watch").then_some(response.clone()),
                    },
                )
                .unwrap(),
                id: child,
            });
        }
        catalog.operations.push(v1::OperationContract {
            id: "fixture.transfer.read_file".into(),
            request: "harmos.storage.v1.ReadFile".into(),
            response: "harmos.storage.v1.FileHash".into(),
            fingerprint: schema::fingerprint(
                "fixture.transfer.read_file",
                &types,
                "harmos.storage.v1.ReadFile",
                "harmos.storage.v1.FileHash",
            )
            .unwrap(),
            visibility: v1::Visibility::Public as i32,
            ..Default::default()
        });
        catalog.requires.push(v1::Requirement {
            id: "storage.read_file".into(),
            fingerprint: schema::host_fingerprint(
                "storage.read_file",
                &v1::Protobuf {
                    descriptors: types.encode_to_vec(),
                    request: "harmos.storage.v1.ReadFile".into(),
                    response: "harmos.storage.v1.FileCompletion".into(),
                    input: None,
                    output: Some("harmos.storage.v1.FileBatch".into()),
                },
            )
            .unwrap(),
        });
        for (method, id, request, response) in [
            ("native_info", "storage.info", "StorageInfoRequest", "StorageInfoResponse"),
            ("native_get_schema", "storage.get_schema", "StorageGetSchemaRequest", "StorageGetSchemaResponse"),
            ("native_set_schema", "storage.set_schema", "StorageSetSchemaRequest", "StorageSetSchemaResponse"),
            ("native_delete_file", "storage.delete_file", "StorageDeleteFileRequest", "StorageDeleteFileResponse"),
        ] {
            let parent = format!("fixture.transfer.{method}");
            let request = format!("harmos.storage.v1.{request}");
            let response = format!("harmos.storage.v1.{response}");
            catalog.operations.push(v1::OperationContract {
                fingerprint: schema::fingerprint(&parent, &types, &request, &response).unwrap(),
                id: parent,
                request: request.clone(),
                response: response.clone(),
                visibility: v1::Visibility::Public as i32,
                ..Default::default()
            });
            catalog.requires.push(v1::Requirement {
                id: id.into(),
                fingerprint: schema::host_fingerprint(
                    id,
                    &v1::Protobuf {
                        descriptors: types.encode_to_vec(),
                        request,
                        response,
                        input: None,
                        output: None,
                    },
                )
                .unwrap(),
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
        let next_policy = Arc::new(std::sync::Mutex::new(None));
        let watch_opened = Arc::new(tokio::sync::Notify::new());
        let service = Service {
            watch_opened: watch_opened.clone(),
            catalog: Arc::new(catalog),
            next_policy: next_policy.clone(),
        };
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(plugin::instance_server::InstanceServer::new(service))
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            address,
            #[cfg(test)]
            next_policy,
            #[cfg(test)]
            watch_opened,
            shutdown,
            task,
        }
    }

    #[cfg(test)]
    pub fn override_next_child(&self, policy: v1::OperationPolicy) {
        *self.next_policy.lock().unwrap() = Some(policy);
    }

    #[cfg(test)]
    pub async fn watch_opened(&self) {
        self.watch_opened.notified().await;
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

        let required = self
            .catalog
            .requires
            .iter()
            .find(|required| {
                required.id.rsplit_once('/').is_some_and(|(_, name)| snake(name) == method)
                    || method == "read_file" && required.id == "storage.read_file"
                    || method.strip_prefix("native_").is_some_and(|name| required.id == format!("storage.{name}"))
            })
            .ok_or_else(|| Status::not_found("Invoke undeclared transfer"))?;
        let watch = method.starts_with("watch_") || method == "read_file";
        let watch_kind = method.to_owned();
        let watch_opened = self.watch_opened.clone();
        let child = v1::ChildRequest {
            call: 1,
            operation: required.id.clone(),
            payload: request.payload,
            fingerprint: required.fingerprint.clone(),
            policy: self.next_policy.lock().unwrap().take(),
            watch,
            max_response_bytes: if watch { 16 * 1024 * 1024 } else { 0 },
            ..Default::default()
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
            if watch
                && send
                    .send(Ok(plugin::InstanceInvokeResponse {
                        frame: Some(plugin::instance_invoke_response::Frame::ChildDemand(v1::ChildDemand { call: 1 })),
                    }))
                    .await
                    .is_err()
            {
                return;
            }
            use sha2::{Digest, Sha256};
            let mut stream_hash = Sha256::new();
            let mut stream_length = 0u64;
            while let Ok(Some(frame)) = input.message().await {
                if let Some(plugin::instance_invoke_request::Frame::Child(child)) = frame.frame {
                    if let Some(v1::child_response::Result::Item(payload)) = &child.result {
                        if watch_kind == "read_file" {
                            let batch = harmos_protocol::storage::FileBatch::decode(payload.as_slice()).unwrap();
                            assert_eq!(batch.offset, stream_length);
                            stream_length += batch.bytes.len() as u64;
                            stream_hash.update(&batch.bytes);
                            if send
                                .send(Ok(plugin::InstanceInvokeResponse {
                                    frame: Some(plugin::instance_invoke_response::Frame::ChildDemand(v1::ChildDemand { call: 1 })),
                                }))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            continue;
                        }
                        let take = match watch_kind.as_str() {
                            "watch_rows" => {
                                let frame = harmos_protocol::context::StorageWatchRowsResponse::decode(payload.as_slice()).unwrap();
                                use harmos_protocol::context::storage_watch_rows_response::Event;
                                if matches!(frame.event, Some(Event::Boundary(_))) {
                                    watch_opened.notify_one();
                                }
                                matches!(
                                    frame.event,
                                    Some(Event::Record(_) | Event::Sample(_) | Event::Object(_) | Event::ResyncRequired(_))
                                )
                            }
                            "watch_file_bytes" => {
                                let frame = harmos_protocol::context::StorageWatchFileBytesResponse::decode(payload.as_slice()).unwrap();
                                frame.event.is_some() || frame.completion.is_some()
                            }
                            "watch_file_production" => {
                                harmos_protocol::context::StorageWatchFileProductionResponse::decode(payload.as_slice())
                                    .unwrap()
                                    .event
                                    .is_some()
                            }
                            "watch_directory_entries" => {
                                harmos_protocol::context::StorageWatchDirectoryEntriesResponse::decode(payload.as_slice())
                                    .unwrap()
                                    .event
                                    .is_some()
                            }
                            _ => unreachable!("declared observation fixture"),
                        };
                        if take {
                            if send
                                .send(Ok(plugin::InstanceInvokeResponse {
                                    frame: Some(plugin::instance_invoke_response::Frame::ChildCancel(v1::ChildCancel { call: 1 })),
                                }))
                                .await
                                .is_err()
                            {
                                return;
                            }
                            let _ = send
                                .send(Ok(plugin::InstanceInvokeResponse {
                                    frame: Some(plugin::instance_invoke_response::Frame::Response(v1::Response {
                                        result: Some(v1::response::Result::Payload(payload.clone())),
                                    })),
                                }))
                                .await;
                            return;
                        }
                        if send
                            .send(Ok(plugin::InstanceInvokeResponse {
                                frame: Some(plugin::instance_invoke_response::Frame::ChildDemand(v1::ChildDemand { call: 1 })),
                            }))
                            .await
                            .is_err()
                        {
                            return;
                        }
                        continue;
                    }
                    if watch_kind == "read_file" && matches!(child.result, Some(v1::child_response::Result::Payload(_))) {
                        let hash = harmos_protocol::storage::FileHash {
                            bytes_read: stream_length,
                            sha256: stream_hash.finalize().to_vec(),
                        };
                        let _ = send
                            .send(Ok(plugin::InstanceInvokeResponse {
                                frame: Some(plugin::instance_invoke_response::Frame::Response(v1::Response {
                                    result: Some(v1::response::Result::Payload(hash.encode_to_vec())),
                                })),
                            }))
                            .await;
                        return;
                    }
                    let result = child.result.map(|result| match result {
                        v1::child_response::Result::Payload(payload) => v1::response::Result::Payload(payload),
                        v1::child_response::Result::Item(_) => unreachable!("unary storage fixture receives no observations"),
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

pub async fn call<O: harmos_operations::execution::operations::ProtobufOperation>(
    operations: &harmos_runtime::operations::OperationManager, request: O::Request,
) -> Result<O::Response, harmos_runtime::Error> {
    let method = snake(O::ID.rsplit('/').next().unwrap());
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

fn snake(method: &str) -> String {
    let mut value = String::new();
    for (index, character) in method.chars().enumerate() {
        if index > 0 && character.is_ascii_uppercase() {
            value.push('_');
        }
        value.push(character.to_ascii_lowercase());
    }
    value
}

/* === Tests === */
