/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::snapshot::SavedSnapshots;
use crate::execution::operations::access::GrantAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::ReplaySafety;
use harmos_protocol::storage as wire;
use harmos_storage::{
    StorageManager,
    query::{columnar::Cancellation, protobuf::required},
};

/* === Structs === */

pub struct ScanArrow;

/* === Implementations === */

impl ProtobufOperation for ScanArrow {
    type Request = wire::ScanArrow;
    type Response = wire::ArrowPage;
    type Endpoints = ();
    const ID: &'static str = "storage.scan_arrow";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "ScanArrow"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] =
        &[Resource::of::<StorageManager>(), Resource::of::<SavedSnapshots>(), Resource::of::<GrantAccess>()];
    fn grant(request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        super::native::grant(Self::ID, request.read.as_ref().and_then(|read| read.source.clone()))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let snapshots = context.resource::<SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                let read: harmos_storage::query::columnar::ArrowScan = required(request.read)?.try_into()?;
                super::native::admit(&context, Self::ID, &read.source)?;
                let mut snapshot = snapshots.read(required(request.snapshot)?)?;
                let page = snapshot.scan_arrow(&read, &cancel).await?;
                super::native::admit(&context, Self::ID, &read.source)?;
                let page: wire::ArrowPage = page.try_into()?;
                super::native::page_bytes(page.buffer_bytes(), read.limits.bytes.get())?;
                snapshot.retain();
                Ok(page)
            },
            cancel.clone(),
        )
        .await
    }
}
