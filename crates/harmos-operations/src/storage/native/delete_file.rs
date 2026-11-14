/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct DeleteFile;

/* === Implementations === */

impl NativeOperation for DeleteFile {
    type Request = super::requests::DeleteFileRequest;
    type Response = super::requests::DeleteFileResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.delete_file";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let mutation: harmos_storage::MutationId =
                request.mutation.ok_or(crate::Error::InvalidInput { field: "mutation", reason: "required" })?;
            let file: harmos_storage::StagedFile = request.file.ok_or(crate::Error::InvalidInput { field: "file", reason: "required" })?;
            let file = storage.delete_file_once(&mutation, &file).await?;
            Ok(Self::Response {
                file: Some(file.into()),
                receipt: Some(super::transfers::receipt(&mutation.into(), None)),
            })
        }
        .await;
        result.map_err(super::admission::failure)
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
