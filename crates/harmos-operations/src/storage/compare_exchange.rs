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
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::ReplaySafety;
use harmos_protocol::storage as wire;
use harmos_storage::{
    StorageManager,
    query::{
        columnar::{Cancellation, ColumnSource},
        protobuf::required,
    },
};

/* === Structs === */

pub struct CompareExchangeObject;

/* === Implementations === */

impl ProtobufOperation for CompareExchangeObject {
    type Request = wire::CompareExchangeObject;
    type Response = wire::CompareExchangeOutcome;
    type Endpoints = ();
    const ID: &'static str = "storage.compare_exchange_object";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "CompareExchangeObject"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(
            Self::ID,
            request.value.as_ref().and_then(|value| value.schema.clone()).map(|schema| wire::ColumnSource {
                kind: wire::ColumnKind::Objects as i32,
                schema: Some(schema),
            }),
        )
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;

        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                let value: harmos_storage::ObjectValue = required(request.value)?.try_into()?;
                let source = ColumnSource {
                    kind: harmos_storage::query::columnar::ColumnKind::Objects,
                    schema: value.schema.clone(),
                };
                super::native::admit(&context, Self::ID, &source)?;
                let schema = required(request.schema)?;
                let key: harmos_storage::SchemaKey = required(schema.key)?.try_into()?;
                if key != value.schema {
                    return Err(crate::Error::InvalidInput {
                        field: "schema",
                        reason: "schema differs from the object value",
                    });
                }
                let graph = required(schema.graph)?.try_into()?;
                let mutation = required(request.mutation)?.try_into()?;
                let expected = request.expected.map(TryInto::try_into).transpose()?;
                Ok(storage.compare_exchange_value_once(&mutation, expected, value, graph).await?.into())
            },
            cancel.clone(),
        )
        .await
    }
}
