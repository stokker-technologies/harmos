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
use harmos_storage::StorageManager;

/* === Structs === */

pub struct AppendFile;

/* === Implementations === */

impl ProtobufOperation for AppendFile {
    type Request = harmos_protocol::storage::AppendFile;
    type Response = harmos_protocol::storage::FileProducer;
    type Endpoints = ();
    const ID: &'static str = "storage.append_file";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::FileProducer> = async {
            let mutation: harmos_storage::MutationId =
                request.mutation.ok_or(crate::Error::InvalidInput { field: "mutation", reason: "required" })?.try_into()?;
            let session: harmos_storage::FileSession =
                request.session.ok_or(crate::Error::InvalidInput { field: "session", reason: "required" })?.try_into()?;
            Ok(storage.append_file_once(&mutation, &session, request.offset, harmos_storage::FileChunk::new(request.bytes)).await?)
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
