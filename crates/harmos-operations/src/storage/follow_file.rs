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

pub struct FollowFile;

/* === Implementations === */

impl ProtobufOperation for FollowFile {
    type Request = harmos_protocol::storage::FollowFile;
    type Response = harmos_protocol::storage::FileCompletion;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::FileEvent>;
    const ID: &'static str = "storage.follow_file";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "FollowFile"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<crate::execution::operations::access::GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<harmos_domain::plugin::Grant>, harmos_domain::MessageError> {
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
        Ok(scope.map(|scope| harmos_domain::plugin::Grant::of(Self::ID, scope)))
    }

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_domain::model::storage::FileCompletion> = async {
            let source: harmos_domain::model::storage::FileSource =
                request.source.ok_or(crate::Error::InvalidInput { field: "source", reason: "required" })?.try_into()?;
            let start: harmos_domain::model::storage::FollowStart =
                request.start.ok_or(crate::Error::InvalidInput { field: "start", reason: "required" })?.try_into()?;
            let grants =
                context.resource::<crate::execution::operations::access::GrantAccess>().map_err(|_| crate::Error::InvalidInput {
                    field: "grants",
                    reason: "resource is missing",
                })?;
            let grant = source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(Self::ID, scope));
            if let Some(grant) = &grant {
                grants.0.admit(grant).map_err(crate::Error::Access)?;
            }
            let mut reader = storage.follow_file(harmos_storage::FileFollow { source, start }).await?;
            while let Some(batch) = reader.next().await? {
                if let Some(grant) = &grant {
                    grants.0.admit(grant).map_err(crate::Error::Access)?;
                }
                storage
                    .clock()
                    .timeout(super::IO_TIMEOUT, output.send((*batch).clone().into()))
                    .await
                    .map_err(|_| super::output_timeout())??;
            }
            Ok(reader.completion()?)
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
