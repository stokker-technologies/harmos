/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_protocol::model::Position;

use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct GetSchema;

/* === Implementations === */

impl NativeOperation for GetSchema {
    type Request = super::requests::GetSchemaRequest;
    type Response = super::requests::GetSchemaResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_schema";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            use crate::MemorySize;
            use prost::Message;
            let key = super::transfers::required(request.key)?;
            let read = snapshots.resolve(&storage, request.at).await?;
            let value = read.get_schema(&key).await?;
            let revision = value.as_ref().map(|value| Position::from(value.position.clone()).encode_to_vec()).unwrap_or_default();
            let schema = value.map(|value| super::requests::SchemaInput {
                key: Some(value.key),
                graph: Some(value.schema.as_ref().clone()),
            });
            let response = Self::Response {
                exists: schema.is_some(),
                schema,
                revision,
                resolved_at: Some(read.cut().clone()),
            };
            if response.memory_size() > storage.settings().response.batch_bytes.get() {
                return Err(harmos_storage::Error::ResourceExhausted { resource: "GetSchema response" }.into());
            }
            Ok(response)
        }
        .await;
        result.map_err(super::admission::failure)
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
