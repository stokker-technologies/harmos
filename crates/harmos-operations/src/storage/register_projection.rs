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
    query::{columnar::Cancellation, protobuf::required},
};

/* === Structs === */

pub struct RegisterProjection;

/* === Implementations === */

impl ProtobufOperation for RegisterProjection {
    type Request = wire::RegisterProjection;
    type Response = wire::DefinitionRevision;
    type Endpoints = ();
    const ID: &'static str = "storage.register_projection";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "RegisterProjection"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.definition.as_ref().and_then(|definition| definition.source.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;

        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                let definition: harmos_storage::query::projection::ProjectionDefinition = required(request.definition)?.try_into()?;
                super::native::admit(&context, Self::ID, &definition.source)?;
                let revision = storage.register_projection(&definition, request.rebuild).await?;
                Ok(wire::DefinitionRevision { revision })
            },
            cancel.clone(),
        )
        .await
    }
}
