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
use harmos_domain::operations::{FailureKind, ReplaySafety};
use harmos_storage::StorageManager;

/* === Structs === */

/// Read the storage frontiers, mutation window and file batch size.
pub struct StorageStatus;

/* === Implementations === */

impl ProtobufOperation for StorageStatus {
    type Request = harmos_protocol::storage::StorageStatus;
    type Response = harmos_protocol::storage::StorageInfo;
    type Endpoints = ();
    const ID: &'static str = "storage.status";
    const POLICY: crate::OperationPolicy = super::POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, _request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        storage.info().map(Into::into).map_err(|error| ServingError::Native(std::sync::Arc::new(crate::Error::from(error))))
    }

    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<crate::Error>().map(super::classify).unwrap_or(FailureKind::Permanent)
    }
}
