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
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_domain::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct Query;

/* === Implementations === */

impl ProtobufOperation for Query {
    type Request = harmos_protocol::storage::Query;
    type Response = harmos_protocol::storage::QueryCompletion;
    type Endpoints = ProtobufOutput<harmos_protocol::storage::QueryOutput>;
    const ID: &'static str = "storage.query";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "Query"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];

    fn grant(request: &Self::Request) -> Result<Option<harmos_domain::plugin::Grant>, harmos_domain::MessageError> {
        let source: harmos_domain::model::storage::Source = request
            .source
            .clone()
            .ok_or_else(|| harmos_domain::MessageError::borrowed("source is required"))?
            .try_into()
            .map_err(|error: harmos_domain::Error| harmos_domain::MessageError::new(error.to_string()))?;
        Ok(Some(harmos_domain::plugin::Grant::of(Self::ID, source.grant_scope())))
    }

    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let grants = context.resource::<GrantAccess>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_domain::model::storage::QueryCompletion> = async {
            let source: harmos_domain::model::storage::Source =
                request.source.ok_or(crate::Error::InvalidInput { field: "source", reason: "required" })?.try_into()?;
            let grant = harmos_domain::plugin::Grant::of(Self::ID, source.grant_scope());
            grants.0.admit(&grant).map_err(crate::Error::Access)?;
            let mut reader = storage
                .query(harmos_storage::Query {
                    at: request.at.ok_or(crate::Error::InvalidInput { field: "at", reason: "required" })?.try_into()?,
                    source,
                    transforms: request.transforms.into_iter().map(TryInto::try_into).collect::<harmos_domain::Result<_>>()?,
                    slice: request.slice.ok_or(crate::Error::InvalidInput { field: "slice", reason: "required" })?.try_into()?,
                })
                .await?;
            while let Some(item) = reader.next().await? {
                grants.0.admit(&grant).map_err(crate::Error::Access)?;
                storage
                    .clock()
                    .timeout(super::IO_TIMEOUT, output.send_with(item.output_memory_size(), || Ok(item.output())))
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
