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

pub struct SetSchema;

/* === Implementations === */

impl ProtobufOperation for SetSchema {
    type Request = harmos_protocol::storage::StorageSetSchemaRequest;
    type Response = harmos_protocol::storage::StorageSetSchemaResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_schema";
    const VISIBILITY: crate::operations::Visibility = crate::operations::Visibility::Protected;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
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
            use prost::Message;
            let expected = if request.create_if_absent && request.expected_revision.is_empty() {
                None
            } else if !request.create_if_absent && !request.expected_revision.is_empty() {
                let value = harmos_protocol::storage::StorageCursor::decode(request.expected_revision.as_slice()).map_err(|_| {
                    crate::Error::InvalidInput {
                        field: "SetSchema",
                        reason: "invalid expected revision",
                    }
                })?;
                if value.encode_to_vec() != request.expected_revision {
                    return Err(crate::Error::InvalidInput {
                        field: "SetSchema",
                        reason: "noncanonical expected revision",
                    });
                }
                Some(value.try_into()?)
            } else {
                return Err(crate::Error::InvalidInput {
                    field: "SetSchema",
                    reason: "provide expected revision or explicit create-if-absent",
                });
            };
            let schema = harmos_protocol::storage::SchemaInput {
                key: Some(key.clone().into()),
                graph: Some(graph.clone().into()),
            };
            let position = storage.set_schema_once(&mutation, key, graph, expected).await?;
            let revision = harmos_protocol::storage::StorageCursor::from(position.clone()).encode_to_vec();
            Ok(Self::Response {
                schema: Some(schema),
                revision,
                receipt: Some(super::transfers::receipt(&mutation.into(), Some(position))),
            })
        }
        .await;
        result.map_err(super::native::failure).and_then(|response| super::native::read_response(&context, response))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
