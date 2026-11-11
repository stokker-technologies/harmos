/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct HashFile;

/* === Implementations === */

impl ProtobufOperation for HashFile {
    type Request = harmos_protocol::storage::HashFile;
    type Response = harmos_protocol::storage::FileHash;
    type Endpoints = ();
    const ID: &'static str = "storage.hash_file";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "HashFile"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    fn grant(request: &Self::Request) -> Result<Option<harmos_operations::plugin::Grant>, harmos_domain::MessageError> {
        let source = request
            .source
            .as_ref()
            .and_then(|source| source.source.as_ref())
            .ok_or_else(|| harmos_domain::MessageError::borrowed("source is required"))?;
        let scope = match source {
            harmos_protocol::storage::file_source::Source::Stored(file) => {
                Some(file.metadata.as_ref().ok_or_else(|| harmos_domain::MessageError::borrowed("metadata is required"))?.id.clone())
            }
            harmos_protocol::storage::file_source::Source::Local(_) => None,
        };
        Ok(scope.map(|scope| harmos_operations::plugin::Grant::of(Self::ID, scope)))
    }

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_domain::model::storage::FileHash> = async {
            let source = request.source.ok_or(crate::Error::InvalidInput { field: "source", reason: "required" })?.try_into()?;
            let range = request.range.ok_or(crate::Error::InvalidInput { field: "range", reason: "required" })?.into();
            Ok(storage.hash_file(harmos_storage::FileRead { source, range }).await?)
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
