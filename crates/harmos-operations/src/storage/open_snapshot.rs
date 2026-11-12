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

pub struct OpenSnapshot;

/* === Implementations === */

impl ProtobufOperation for OpenSnapshot {
    type Request = wire::OpenSnapshot;
    type Response = wire::SavedSnapshot;
    type Endpoints = ();
    const ID: &'static str = "storage.open_snapshot";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "OpenSnapshot"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>(), Resource::of::<SavedSnapshots>()];

    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshots = context.resource::<SavedSnapshots>().map_err(|_| ServingError::Catalog)?;
        let cancel = Cancellation::default();
        super::native::run(
            &context,
            async {
                let at = required(request.at)?.try_into()?;
                let permit = snapshots.reserve()?;
                let snapshot = storage.snapshot(&at).await?;
                Ok(snapshots.insert(snapshot, permit)?)
            },
            cancel.clone(),
        )
        .await
    }
}
