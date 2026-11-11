/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, ProtobufOperation, Resource, ServingError};
use harmos_domain::operations::{FailureKind, ReplaySafety};
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Structs === */

pub struct AbandonFile;

/* === Implementations === */

impl ProtobufOperation for AbandonFile {
    type Request = harmos_protocol::storage::AbandonFile;
    type Response = harmos_protocol::storage::FileProgress;
    type Endpoints = ();
    const ID: &'static str = "storage.abandon_file";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager<ErasedCatalog>>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager<ErasedCatalog>>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::FileProgress> = async {
            let mutation: harmos_storage::MutationId =
                request.mutation.ok_or(crate::Error::InvalidInput { field: "mutation", reason: "required" })?.try_into()?;
            let session: harmos_storage::FileSession =
                request.session.ok_or(crate::Error::InvalidInput { field: "session", reason: "required" })?.try_into()?;
            Ok(storage.abandon_file_once(&mutation, &session).await?)
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
