/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Closed Catalog Of Storage Operation Declarations. The Runtime Binds
// Each Declaration To Its Storage Handler.

/* === Modules === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod definitions;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod file_access;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod file_watches;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod lists;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod pages;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod views;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod watch_rows;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod client;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub mod context;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod transfers;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub(crate) mod append;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod delete_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod get_schema;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod info;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod registration;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod set_schema;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod object;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod snapshot;

/* === Re-Exports === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use client::{
    AdvanceProjection, AppendFileBytes, AppendObjectRevisions, AppendRecords, AppendSamples, Capabilities, CaptureFile,
    CreateFileProduction, CreateFileWriter, CreateSnapshot, DeleteFileProduction, DeleteSnapshot, GetFileBytes, GetFileHash,
    GetFileProduction, GetIndexDefinition, GetObject, GetProjectionDefinition, GetProjectionStatus, ListColumnRows, ListDirectoryEntries,
    ListIndexEntries, ListProjectionRows, ListRows, PublishFileProduction, SetIndexDefinition, SetObject, SetProjectionDefinition,
    WatchDirectoryEntries, WatchFileBytes, WatchFileProduction, WatchRows,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use snapshot::SavedSnapshots;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use delete_file::DeleteFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use get_schema::GetSchema;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use info::Info;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use registration::storage_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use set_schema::SetSchema;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports

use harmos_operations::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};

/* === Constants === */

pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(60),
    attempt_timeout: Duration::from_secs(15),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::OnTransient {
        total_attempts: 4,
        backoff: Backoff::Exponential {
            initial: Duration::from_millis(250),
            maximum: Duration::from_secs(2),
            jitter: Jitter::Deterministic,
        },
    },
};

pub const STREAM_POLICY: OperationPolicy = OperationPolicy {
    attempt_timeout: POLICY.timeout,
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
    ..POLICY
};

pub const IO_POLICY: OperationPolicy = OperationPolicy {
    timeout: harmos_operations::operations::UNBOUNDED,
    attempt_timeout: harmos_operations::operations::UNBOUNDED,
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
fn classify(error: &crate::Error) -> harmos_operations::operations::FailureKind {
    match error {
        crate::Error::Storage(source) if source.backend_failure() == Some(harmos_storage::BackendFailure::Transient) => {
            harmos_operations::operations::FailureKind::Transient
        }
        _ => harmos_operations::operations::FailureKind::Permanent,
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod tests {
    use super::classify;
    use harmos_operations::operations::FailureKind;
    use harmos_storage::{BackendFailure, Error as StorageError};

    #[test]
    fn persistence_preserves_failure_kind() {
        for (backend, expected) in
            [(BackendFailure::Transient, FailureKind::Transient), (BackendFailure::Permanent, FailureKind::Permanent)]
        {
            let source = StorageError::backend("write", backend, std::io::Error::other("backend failed"));
            let error = StorageError::Persistence { source: std::sync::Arc::new(source) };
            assert_eq!(classify(&crate::Error::Storage(error)), expected);
        }
    }
}
