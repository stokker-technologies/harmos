/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use harmos_operations::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

pub struct SetSchema;

/* === Implementations === */

impl NativeOperation for SetSchema {
    type Request = super::requests::SetSchemaRequest;
    type Response = super::requests::SetSchemaResponse;
    type Endpoints = ();
    const ID: &'static str = "storage.set_schema";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let result: crate::Result<Self::Response> = async {
            let mutation: harmos_storage::MutationId =
                request.mutation.ok_or(crate::Error::InvalidInput { field: "mutation", reason: "required" })?;
            let schema = request.schema.ok_or(crate::Error::InvalidInput { field: "schema", reason: "required" })?;
            let key: harmos_storage::SchemaKey = schema.key.ok_or(crate::Error::InvalidInput { field: "key", reason: "required" })?;
            let graph: harmos_domain::schema::graph::Schema =
                schema.graph.ok_or(crate::Error::InvalidInput { field: "graph", reason: "required" })?;
            graph.prepare(&harmos_domain::config::SchemaSettings::default()).map_err(|_| crate::Error::InvalidInput {
                field: "storage schema",
                reason: "invalid finite schema graph",
            })?;
            use prost::Message;
            let expected = if request.create_if_absent && request.expected_revision.is_empty() {
                None
            } else if !request.create_if_absent && !request.expected_revision.is_empty() {
                let value = harmos_protocol::v1::Position::decode(request.expected_revision.as_slice()).map_err(|_| {
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
            let schema = super::requests::SchemaInput {
                key: Some(key.clone()),
                graph: Some(graph.clone()),
            };
            let position = storage.set_schema_once(&mutation, key, graph, expected).await?;
            let revision = harmos_protocol::v1::Position::from(position.clone()).encode_to_vec();
            Ok(Self::Response {
                schema: Some(schema),
                revision,
                receipt: Some(super::transfers::receipt(&mutation, Some(position))),
            })
        }
        .await;
        result.map_err(super::admission::failure).and_then(|response| super::admission::read_response(&context, response))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}
