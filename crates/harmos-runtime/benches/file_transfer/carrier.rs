/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Drive Client and Plugin Storage Transfers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::transfer_plugin;
use harmos_operations::storage::{self as operations, context};
use harmos_protocol::{context as child, storage as wire, v1 as client};
use harmos_storage::{
    Error, FileProducer, FileReference, Result, StorageFuture,
    bench_support::file_transfer::{Carrier, Range},
};
use std::{
    num::NonZeroUsize,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Transport {
    pub operations: harmos_runtime::operations::OperationManager,
    pub channel: tonic::transport::Channel,
    pub window: String,
    pub plugin: bool,
    nonce: AtomicU64,
    integrity: Mutex<(u64, [u8; 32])>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct UploadFrame {
    #[prost(message, optional, tag = "1")]
    request: Option<wire::WriteFile>,
    #[prost(message, optional, tag = "2")]
    item: Option<wire::FileBatch>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Transport {
    pub fn new(
        operations: harmos_runtime::operations::OperationManager, channel: tonic::transport::Channel, window: String, plugin: bool,
    ) -> Self {
        Self {
            operations,
            channel,
            window,
            plugin,
            nonce: AtomicU64::new(0),
            integrity: Mutex::new((0, [0; 32])),
        }
    }
    fn mutation(&self) -> Option<wire::MutationId> {
        Some(wire::MutationId {
            window: self.window.clone(),
            key: format!("{}-{}", self.name(), self.nonce.fetch_add(1, Ordering::Relaxed)),
        })
    }
    fn metadata(&self, length: u64, sha256: [u8; 32]) -> wire::FileMetadata {
        wire::FileMetadata {
            id: format!("{}-{}", self.name(), self.nonce.fetch_add(1, Ordering::Relaxed)),
            media_type: "application/octet-stream".into(),
            expected_length: Some(length),
            expected_sha256: Some(sha256.to_vec()),
            ..Default::default()
        }
    }
}

impl Carrier for Transport {
    fn name(&self) -> &str {
        if self.plugin { "plugin" } else { "client" }
    }

    fn create(&self, length: u64, sha256: [u8; 32]) -> StorageFuture<'_, FileProducer> {
        Box::pin(async move {
            *self.integrity.lock().unwrap() = (length, sha256);
            let metadata = Some(self.metadata(length, sha256));
            let mutation = self.mutation();
            let production = if self.plugin {
                transfer_plugin::call::<context::CreateFileProduction>(
                    &self.operations,
                    child::StorageCreateFileProductionRequest { metadata, mutation },
                )
                .await
                .map_err(|e| failure("CreateFileProduction", e))?
                .production
            } else {
                transfer_plugin::client_call::<operations::CreateFileProduction>(
                    &self.channel,
                    client::StorageCreateFileProductionRequest { metadata, mutation },
                )
                .await
                .map_err(|e| failure("CreateFileProduction", e))?
                .production
            };
            producer(production.expect("production response"))
        })
    }

    fn append<'a>(&'a self, production: &'a FileProducer, offset: u64, data: &'a [u8]) -> StorageFuture<'a, FileProducer> {
        Box::pin(async move {
            let session = Some(production.session.clone().into());
            let mutation = self.mutation();
            let data = data.to_vec();
            let production = if self.plugin {
                transfer_plugin::call::<context::AppendFileBytes>(
                    &self.operations,
                    child::StorageAppendFileBytesRequest { session, mutation, offset, data },
                )
                .await
                .map_err(|e| failure("AppendFileBytes", e))?
                .production
            } else {
                transfer_plugin::client_call::<operations::AppendFileBytes>(
                    &self.channel,
                    client::StorageAppendFileBytesRequest { session, mutation, offset, data },
                )
                .await
                .map_err(|e| failure("AppendFileBytes", e))?
                .production
            };
            producer(production.expect("production response"))
        })
    }

    fn publish<'a>(&'a self, production: &'a FileProducer) -> StorageFuture<'a, FileReference> {
        Box::pin(async move {
            let (length, digest) = *self.integrity.lock().unwrap();
            let sha256 = digest.to_vec();
            let session = Some(production.session.clone().into());
            let mutation = self.mutation();
            let file = if self.plugin {
                transfer_plugin::call::<context::PublishFileProduction>(
                    &self.operations,
                    child::StoragePublishFileProductionRequest { session, mutation, length, sha256 },
                )
                .await
                .map_err(|e| failure("PublishFileProduction", e))?
                .file
            } else {
                transfer_plugin::client_call::<operations::PublishFileProduction>(
                    &self.channel,
                    client::StoragePublishFileProductionRequest { session, mutation, length, sha256 },
                )
                .await
                .map_err(|e| failure("PublishFileProduction", e))?
                .file
            };
            file.expect("published reference").try_into().map_err(Error::from)
        })
    }

    fn get<'a>(&'a self, file: &'a FileReference, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Range> {
        Box::pin(async move {
            let file = Some(file.clone().into());
            let max_bytes = maximum.get() as u64;
            let (data, total_length, sha256) = if self.plugin {
                let range = transfer_plugin::call::<context::GetFileBytes>(
                    &self.operations,
                    child::StorageGetFileBytesRequest { file, offset, max_bytes, at: None },
                )
                .await
                .map_err(|e| failure("GetFileBytes", e))?;
                (range.data, range.total_length, range.sha256)
            } else {
                let range = transfer_plugin::client_call::<operations::GetFileBytes>(
                    &self.channel,
                    client::StorageGetFileBytesRequest { file, offset, max_bytes, at: None },
                )
                .await
                .map_err(|e| failure("GetFileBytes", e))?;
                (range.data, range.total_length, range.sha256)
            };
            Ok(Range {
                data,
                total_length,
                sha256: sha256.try_into().expect("SHA-256 response"),
            })
        })
    }

    fn upload_stream(&self, data: std::sync::Arc<[u8]>, chunk_bytes: NonZeroUsize, sha256: [u8; 32]) -> StorageFuture<'_, FileReference> {
        Box::pin(async move {
            if self.plugin {
                return Err(unsupported());
            }
            let metadata = self.metadata(data.len() as u64, sha256);
            let opening = UploadFrame {
                request: Some(wire::WriteFile { metadata: Some(metadata) }),
                item: None,
            };
            let chunks = futures_util::stream::unfold((data, 0usize), move |(data, offset)| async move {
                if offset >= data.len() {
                    return None;
                }
                let end = (offset + chunk_bytes.get()).min(data.len());
                let item = UploadFrame {
                    request: None,
                    item: Some(wire::FileBatch {
                        offset: offset as u64,
                        bytes: data[offset..end].to_vec(),
                    }),
                };
                Some((item, (data, end)))
            });
            use futures_util::StreamExt;
            let stream = futures_util::stream::once(async { opening }).chain(chunks);
            let mut client = tonic::client::Grpc::new(self.channel.clone());
            client.ready().await.map_err(|e| failure("WriteFile", e))?;
            let path = tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/WriteFile");
            let response: tonic::Response<wire::FileReference> = client
                .client_streaming(tonic::Request::new(stream), path, tonic_prost::ProstCodec::<UploadFrame, wire::FileReference>::default())
                .await
                .map_err(|e| failure("WriteFile", e))?;
            response.into_inner().try_into().map_err(Error::from)
        })
    }

    fn download_stream<'a>(&'a self, file: &'a FileReference) -> StorageFuture<'a, harmos_storage::FileHash> {
        Box::pin(async move {
            let request = wire::ReadFile {
                source: Some(wire::FileSource {
                    source: Some(wire::file_source::Source::Stored(file.clone().into())),
                }),
                range: Some(wire::ByteRange { offset: 0, limit: None }),
            };
            if self.plugin {
                use prost::Message;
                let response = self
                    .operations
                    .call_encoded("fixture.transfer.read_file", request.encode_to_vec())
                    .await
                    .map_err(|e| failure("ReadFile", e))?;
                let hash = wire::FileHash::decode(response.as_slice()).map_err(|e| failure("ReadFile", e))?;
                return Ok(harmos_storage::FileHash {
                    bytes_read: hash.bytes_read,
                    sha256: hash.sha256.try_into().expect("SHA-256"),
                });
            }
            let mut client = tonic::client::Grpc::new(self.channel.clone());
            client.ready().await.map_err(|e| failure("ReadFile", e))?;
            let path = tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/ReadFile");
            let response: tonic::Response<tonic::Streaming<wire::FileBatch>> = client
                .server_streaming(tonic::Request::new(request), path, tonic_prost::ProstCodec::<wire::ReadFile, wire::FileBatch>::default())
                .await
                .map_err(|e| failure("ReadFile", e))?;
            let mut stream = response.into_inner();
            use sha2::{Digest, Sha256};
            let mut length = 0u64;
            let mut hash = Sha256::new();
            while let Some(item) = stream.message().await.map_err(|e| failure("ReadFile", e))? {
                assert_eq!(item.offset, length);
                length += item.bytes.len() as u64;
                hash.update(&item.bytes);
            }
            assert_eq!(file.length, length);
            Ok(harmos_storage::FileHash {
                bytes_read: length,
                sha256: hash.finalize().into(),
            })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn producer(value: wire::FileProducer) -> Result<FileProducer> {
    Ok(FileProducer {
        session: value.session.expect("writer session").try_into()?,
        length: value.length,
    })
}

fn failure(action: &str, source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: action.into(),
        source: std::sync::Arc::new(source),
    }
}

fn unsupported() -> Error {
    Error::backend(
        "stream file through plugin child calls",
        harmos_storage::BackendFailure::Unsupported,
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "plugin child calls reject input streams; retained streaming upload is unavailable",
        ),
    )
}

/* === Tests === */
