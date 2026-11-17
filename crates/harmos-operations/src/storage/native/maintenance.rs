/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Backend Maintenance Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::error;
use std::mem;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::MemorySize;
use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};
use crate::operations::{FailureKind, OperationPolicy, RetryPolicy};
use harmos_storage::{StorageBackend, StorageCursor, StorageManager};

/* === Constants === */

const POLICY: OperationPolicy = OperationPolicy {
    retries: RetryPolicy::Never,
    ..super::POLICY
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct CopyBackend;

pub struct DisposeBackend;

#[derive(Clone)]
pub struct CopyBackendRequest {
    pub at: StorageCursor,
    pub target: Arc<dyn StorageBackend>,
}

#[derive(Clone)]
pub struct DisposeBackendRequest {
    pub target: Arc<dyn StorageBackend>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for CopyBackendRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.at.memory_size())
    }
}

impl MemorySize for DisposeBackendRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

impl NativeOperation for CopyBackend {
    type Request = CopyBackendRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "storage.copy_backend";
    const POLICY: OperationPolicy = POLICY;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StorageManager>()];

    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<(), ServingError> {
        let storage = context.resource::<StorageManager>().map_err(|_| ServingError::Catalog)?;
        let snapshot = storage.snapshot(&request.at).await.map_err(crate::Error::from).map_err(super::admission::failure)?;
        snapshot.copy_to(request.target).await.map_err(crate::Error::from).map_err(super::admission::failure)
    }

    fn classify(error: &(dyn error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

impl NativeOperation for DisposeBackend {
    type Request = DisposeBackendRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "storage.dispose_backend";
    const POLICY: OperationPolicy = POLICY;

    async fn run(_: ExecutionContext, request: Self::Request, _: &()) -> Result<(), ServingError> {
        request.target.dispose().await.map_err(crate::Error::from).map_err(super::admission::failure)
    }

    fn classify(error: &(dyn error::Error + Send + Sync + 'static)) -> FailureKind {
        super::transfers::classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
