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
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct GetSchema;

/* === Implementations === */

impl ProtobufOperation for GetSchema {
    type Request = harmos_protocol::storage::StorageGetSchemaRequest;
    type Response = harmos_protocol::storage::StorageGetSchemaResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.get_schema";
    const VISIBILITY: crate::operations::Visibility = crate::operations::Visibility::Protected;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<super::SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<super::SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            use prost::Message;
            let key = super::transfers::required(request.key)?.try_into()?;
            let read = snapshots.resolve(&storage, request.at).await?;
            let value = read.get_schema(&key).await?;
            let revision = value
                .as_ref()
                .map(|value| harmos_protocol::storage::StorageCursor::from(value.position.clone()).encode_to_vec())
                .unwrap_or_default();
            let schema = value.map(|value| harmos_protocol::storage::SchemaInput {
                key: Some(value.key.into()),
                graph: Some(value.schema.as_ref().clone().into()),
            });
            let response = Self::Response {
                exists: schema.is_some(),
                schema,
                revision,
                resolved_at: Some(read.cut().clone()),
            };
            if response.encoded_len() > storage.settings().response.batch_bytes.get() {
                return Err(harmos_storage::Error::ResourceExhausted { resource: "GetSchema response" }.into());
            }
            Ok(response)
        }
        .await;
        result.map_err(super::native::failure)
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
