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

pub struct WriteSchema;

/* === Implementations === */

impl ProtobufOperation for WriteSchema {
    type Request = harmos_protocol::storage::WriteSchema;
    type Response = harmos_protocol::storage::StorageCursor;
    type Endpoints = ();
    const ID: &'static str = "storage.write_schema";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager<ErasedCatalog>>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager<ErasedCatalog>>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<harmos_storage::StorageCursor> = async {
            let mutation: harmos_storage::MutationId =
                request.mutation.ok_or(crate::Error::InvalidInput { field: "mutation", reason: "required" })?.try_into()?;
            let schema = request.schema.ok_or(crate::Error::InvalidInput { field: "schema", reason: "required" })?;
            let key: harmos_storage::SchemaKey =
                schema.key.ok_or(crate::Error::InvalidInput { field: "key", reason: "required" })?.try_into()?;
            let graph: harmos_domain::schema::graph::Schema =
                schema.graph.ok_or(crate::Error::InvalidInput { field: "graph", reason: "required" })?.try_into()?;
            graph.prepare(&harmos_domain::config::SchemaSettings::default()).map_err(|_| crate::Error::InvalidInput {
                field: "storage schema",
                reason: "invalid finite schema graph",
            })?;
            Ok(storage.write_schema_once(&mutation, key, graph).await?)
        }
        .await;
        result.map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
