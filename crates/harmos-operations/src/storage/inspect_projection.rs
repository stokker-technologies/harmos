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

pub struct InspectProjection;

/* === Implementations === */

impl ProtobufOperation for InspectProjection {
    type Request = wire::InspectProjection;
    type Response = wire::ProjectionState;
    type Endpoints = ();
    const ID: &'static str = "storage.inspect_projection";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "InspectProjection"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.source.clone())
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;

        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                let source: ColumnSource = required(request.source)?.try_into()?;
                super::native::admit(&context, Self::ID, &source)?;
                let name = harmos_storage::Identifier::new(request.name).map_err(|_| crate::Error::InvalidInput {
                    field: "projection",
                    reason: "invalid name",
                })?;
                let definition = storage.get_projection_definition(&name, Some(request.revision)).await?.0;
                super::native::source_matches(&source, &definition.source)?;
                let frontier = required(request.frontier)?.try_into()?;
                let status = storage.get_projection_status(&name, request.revision, Some(&frontier)).await?;
                Ok(wire::ProjectionState { status: status.map(Into::into) })
            },
            cancel.clone(),
        )
        .await
    }
}
