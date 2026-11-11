/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::access::GrantAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufInput, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Structs === */

pub struct WriteFile;

/* === Implementations === */

impl ProtobufOperation for WriteFile {
    type Request = harmos_protocol::storage::WriteFile;
    type Response = harmos_protocol::storage::FileReference;
    type Endpoints = ProtobufInput<harmos_protocol::storage::FileBatch>;
    const ID: &'static str = "storage.write_file";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "WriteFile"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager<ErasedCatalog>>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<harmos_domain::plugin::Grant>, harmos_domain::MessageError> {
        let metadata = request.metadata.as_ref().ok_or_else(|| harmos_domain::MessageError::borrowed("metadata is required"))?;
        Ok(Some(harmos_domain::plugin::Grant::of(Self::ID, metadata.id.clone())))
    }

    async fn run(context: ExecutionContext, request: Self::Request, input: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager<ErasedCatalog>>().map_err(|_| ServingError::Catalog)?;
        let grants = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::FileReference> = async {
            let metadata: harmos_storage::FileMetadata =
                request.metadata.ok_or(crate::Error::InvalidInput { field: "metadata", reason: "required" })?.try_into()?;
            let grant = harmos_domain::plugin::Grant::of(Self::ID, metadata.id.to_string());
            grants.0.admit(&grant).map_err(crate::Error::Access)?;
            storage.retry_transient_persistence();
            let mut file = storage.create_file(metadata).await?;
            let mut offset = 0u64;
            while let Some(batch) = input.recv().await {
                let (batch, _claims) = batch.into_owned_request()?;
                grants.0.admit(&grant).map_err(crate::Error::Access)?;
                if batch.offset != offset || batch.bytes.is_empty() {
                    return Err(crate::Error::InvalidInput {
                        field: "file batch",
                        reason: "nonempty batches must have contiguous byte offsets",
                    });
                }
                let next = offset.checked_add(batch.bytes.len() as u64).ok_or(crate::Error::InvalidInput {
                    field: "file batch offset",
                    reason: "byte offset overflow",
                })?;
                file.write_all(batch.bytes).await?;
                offset = next;
            }
            grants.0.admit(&grant).map_err(crate::Error::Access)?;
            let reference = file.finish().await?;
            storage.await_durable(&reference.position).await?;
            Ok(reference)
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::execution::operations::FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(crate::execution::operations::FailureKind::Permanent)
    }
}
