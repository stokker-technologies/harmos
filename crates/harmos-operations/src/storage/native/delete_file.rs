/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use std::error;

use super::requests::{DeleteFileRequest, DeleteFileResponse};
use super::snapshot::SavedSnapshots;
use crate::OperationPolicy;
use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::{MutationId, StagedFile, StorageManager};

/* === Structs === */

pub struct DeleteFile;

/* === Implementations === */

impl NativeOperation for DeleteFile {
    type Request = DeleteFileRequest;
    type Response = DeleteFileResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.delete_file";
    const POLICY: OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let mutation: MutationId = request.mutation.ok_or(crate::Error::InvalidInput { field: "mutation", reason: "required" })?;
            let file: StagedFile = request.file.ok_or(crate::Error::InvalidInput { field: "file", reason: "required" })?;
            let _issuance = snapshots.issuance.lock().await;
            let file = storage.delete_file_once(&mutation, &file).await?;
            snapshots.invalidate_implicit();
            Ok(Self::Response {
                file: Some(file),
                receipt: Some(super::transfers::receipt(&mutation, None)),
            })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
