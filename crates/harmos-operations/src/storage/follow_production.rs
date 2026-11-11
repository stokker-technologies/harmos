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
use harmos_storage::StorageManager;

/* === Structs === */

pub struct FollowProduction;

/* === Implementations === */

impl ProtobufOperation for FollowProduction {
    type Request = harmos_protocol::storage::FollowProduction;
    type Response = harmos_protocol::storage::FileProgress;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::FileBatch>;
    const ID: &'static str = "storage.follow_production";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "FollowProduction"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<crate::execution::operations::access::GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<harmos_domain::plugin::Grant>, harmos_domain::MessageError> {
        let file = request.file.as_ref().ok_or_else(|| harmos_domain::MessageError::borrowed("file is required"))?;
        Ok(Some(harmos_domain::plugin::Grant::of(Self::ID, file.id.clone())))
    }

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_domain::model::storage::FileProgress> = async {
            let file: harmos_domain::model::storage::StagedFile =
                request.file.ok_or(crate::Error::InvalidInput { field: "file", reason: "required" })?.try_into()?;
            let grants =
                context.resource::<crate::execution::operations::access::GrantAccess>().map_err(|_| crate::Error::InvalidInput {
                    field: "grants",
                    reason: "resource is missing",
                })?;
            let grant = Some(harmos_domain::plugin::Grant::of(Self::ID, file.id.to_string()));
            if let Some(grant) = &grant {
                grants.0.admit(grant).map_err(crate::Error::Access)?;
            }
            let mut reader = storage.follow_production(&file, request.offset).await?;
            while let Some(batch) = reader.next_batch(storage.settings().response.batch_bytes).await? {
                if let Some(grant) = &grant {
                    grants.0.admit(grant).map_err(crate::Error::Access)?;
                }
                storage
                    .clock()
                    .timeout(
                        super::IO_TIMEOUT,
                        output.send(harmos_protocol::storage::FileBatch {
                            offset: batch.offset(),
                            bytes: batch.bytes().to_vec(),
                        }),
                    )
                    .await
                    .map_err(|_| super::output_timeout())??;
            }
            Ok(reader.progress())
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
