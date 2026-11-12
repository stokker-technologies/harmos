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
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::ReplaySafety;
use harmos_protocol::storage as wire;
use harmos_storage::{
    StorageManager,
    query::{columnar::Cancellation, protobuf::required},
};

/* === Structs === */

pub struct ReleaseSnapshot;

/* === Implementations === */

impl ProtobufOperation for ReleaseSnapshot {
    type Request = wire::ReleaseSnapshot;
    type Response = wire::Empty;
    type Endpoints = ();
    const ID: &'static str = "storage.release_snapshot";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "ReleaseSnapshot"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let snapshots = context.resource::<SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                snapshots.release(&required(request.snapshot)?)?;
                Ok(wire::Empty {})
            },
            cancel.clone(),
        )
        .await
    }
}
