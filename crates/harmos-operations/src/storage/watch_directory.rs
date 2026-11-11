/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_domain::operations::{FailureKind, ReplaySafety};
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Structs === */

pub struct WatchDirectory;

/* === Implementations === */

impl ProtobufOperation for WatchDirectory {
    type Request = harmos_protocol::storage::WatchDirectory;
    type Response = harmos_protocol::storage::DirectoryCompletion;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::DirectoryEvent>;
    const ID: &'static str = "storage.watch_directory";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "WatchDirectory"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager<ErasedCatalog>>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager<ErasedCatalog>>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_domain::model::storage::DirectoryCompletion> = async {
            let selection = request.selection.ok_or(crate::Error::InvalidInput { field: "selection", reason: "required" })?.try_into()?;
            let mut reader = storage.watch_directory(selection, request.include_existing).await?;
            let mut entries = 0_u64;
            while let Some(item) = reader.next().await? {
                if matches!(
                    &*item,
                    harmos_domain::model::storage::DirectoryEvent::Existing(_)
                        | harmos_domain::model::storage::DirectoryEvent::Created(_)
                        | harmos_domain::model::storage::DirectoryEvent::Modified(_)
                ) {
                    entries = entries.checked_add(1).ok_or(crate::Error::ResourceExhausted { resource: "directory observation count" })?;
                }
                storage
                    .clock()
                    .timeout(super::IO_TIMEOUT, output.send((*item).clone().into()))
                    .await
                    .map_err(|_| super::output_timeout())??;
            }
            Ok(harmos_domain::model::storage::DirectoryCompletion { entries })
        }
        .await;
        result.map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
