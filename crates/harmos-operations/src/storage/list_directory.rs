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
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct ListDirectory;

/* === Implementations === */

impl ProtobufOperation for ListDirectory {
    type Request = harmos_protocol::storage::ListDirectory;
    type Response = harmos_protocol::storage::DirectoryCompletion;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::DirectoryEntry>;
    const ID: &'static str = "storage.list_directory";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "ListDirectory"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_domain::model::storage::DirectoryCompletion> = async {
            let selection = request.selection.ok_or(crate::Error::InvalidInput { field: "selection", reason: "required" })?.try_into()?;
            let mut reader = storage.list_directory(selection).await?;
            while let Some(item) = reader.next().await? {
                storage.clock().timeout(super::IO_TIMEOUT, output.send((*item).clone())).await.map_err(|_| super::output_timeout())??;
            }
            Ok(reader.completion()?)
        }
        .await;
        result.map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
