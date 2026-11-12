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

pub struct ReadProjection;

/* === Implementations === */

impl ProtobufOperation for ReadProjection {
    type Request = wire::ReadProjection;
    type Response = wire::ProjectionPage;
    type Endpoints = ();
    const ID: &'static str = "storage.read_projection";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "ReadProjection"));
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
                let read: harmos_storage::query::projection::ProjectionRead = required(request.read)?.try_into()?;
                let definition = storage.projection_definition_at(&read.name, read.revision).await?;
                super::native::source_matches(&source, &definition.source)?;
                let page = storage.read_projection(&read).await?;
                super::native::admit(&context, Self::ID, &source)?;
                let page: wire::ProjectionPage = page.try_into()?;
                super::native::page_bytes(page.buffer_bytes(), read.limits.bytes.get())?;
                Ok(page)
            },
            cancel.clone(),
        )
        .await
    }
}
