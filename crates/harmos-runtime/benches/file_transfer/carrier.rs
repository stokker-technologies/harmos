/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Drive Native Storage Operation Transfers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_operations::storage as operations;
use harmos_runtime::operations::OperationManager;
use harmos_storage::{
    Error, FileMetadata, FileProducer, FileReference, Identifier, MutationId, StorageFuture,
    bench_support::file_transfer::{Carrier, Range},
};
use std::{
    error::Error as StdError,
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Native {
    operations: OperationManager,
    window: Identifier,
    nonce: AtomicU64,
    integrity: Mutex<(u64, [u8; 32])>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Native {
    pub fn new(operations: OperationManager, window: Identifier) -> Self {
        Self {
            operations,
            window,
            nonce: AtomicU64::new(0),
            integrity: Mutex::new((0, [0; 32])),
        }
    }
    fn mutation(&self) -> MutationId {
        MutationId {
            window: self.window.clone(),
            key: Identifier::new(format!("transfer-{}", self.nonce.fetch_add(1, Ordering::Relaxed))).unwrap(),
        }
    }
}
impl Carrier for Native {
    fn name(&self) -> &'static str {
        "native"
    }
    fn create(&self, length: u64, sha256: [u8; 32]) -> StorageFuture<'_, FileProducer> {
        Box::pin(async move {
            *self.integrity.lock().unwrap() = (length, sha256);
            let mutation = self.mutation();
            let metadata = FileMetadata {
                id: mutation.key.clone(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: Some(length),
                expected_sha256: Some(sha256),
            };
            let response = self
                .operations
                .call_native::<operations::CreateFileProduction>(operations::CreateFileProductionRequest {
                    metadata: Some(metadata),
                    mutation: Some(mutation),
                })
                .await
                .map_err(|source| failure("Create File Production", source))?;
            Ok(response.production.expect("created production"))
        })
    }
    fn append<'a>(&'a self, production: &'a FileProducer, offset: u64, data: &'a [u8]) -> StorageFuture<'a, FileProducer> {
        Box::pin(async move {
            let response = self
                .operations
                .call_native::<operations::AppendFileBytes>(operations::AppendFileBytesRequest {
                    session: Some(production.session.clone()),
                    mutation: Some(self.mutation()),
                    offset,
                    data: data.to_vec(),
                })
                .await
                .map_err(|source| failure("Append File Bytes", source))?;
            Ok(response.production.expect("appended production"))
        })
    }
    fn publish<'a>(&'a self, production: &'a FileProducer) -> StorageFuture<'a, FileReference> {
        Box::pin(async move {
            let (length, sha256) = *self.integrity.lock().unwrap();
            let response = self
                .operations
                .call_native::<operations::PublishFileProduction>(operations::PublishFileProductionRequest {
                    session: Some(production.session.clone()),
                    mutation: Some(self.mutation()),
                    length,
                    sha256: sha256.to_vec(),
                })
                .await
                .map_err(|source| failure("Publish File Production", source))?;
            Ok(response.file.expect("published reference"))
        })
    }
    fn get<'a>(&'a self, file: &'a FileReference, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Range> {
        Box::pin(async move {
            let response = self
                .operations
                .call_native::<operations::GetFileBytes>(operations::GetFileBytesRequest {
                    file: Some(file.clone()),
                    offset,
                    max_bytes: maximum.get() as u64,
                    at: None,
                })
                .await
                .map_err(|source| failure("Get File Bytes", source))?;
            Ok(Range {
                data: response.data,
                total_length: response.total_length,
                sha256: response.sha256.try_into().expect("content digest"),
            })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn failure(action: &str, source: impl StdError + Send + Sync + 'static) -> Error {
    Error::OperationFailed {
        operation: action.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
