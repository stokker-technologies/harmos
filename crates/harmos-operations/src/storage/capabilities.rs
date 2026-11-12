/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::operations::ReplaySafety;
use harmos_protocol::storage as wire;
use harmos_storage::{StorageManager, query::columnar::Cancellation};

/* === Structs === */

pub struct Capabilities;

/* === Implementations === */

impl ProtobufOperation for Capabilities {
    type Request = wire::Capabilities;
    type Response = wire::StorageCapabilities;
    type Endpoints = ();
    const ID: &'static str = "storage.capabilities";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.storage.v1", "Storage", "Capabilities"));
    const POLICY: crate::OperationPolicy = super::IO_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, _request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;

        let cancel = Cancellation::default();
        super::native::run(&context, async { Ok(storage.get_storage_capabilities().into()) }, cancel.clone()).await
    }
}
