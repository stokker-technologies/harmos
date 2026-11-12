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

mod contracts;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod definitions;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod pages;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod views;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod client;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub mod context;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod transfers;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod capture_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod delete_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub(crate) mod exchange;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod follow_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod follow_objects;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod follow_production;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod follow_records;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod follow_stream;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod hash_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod list_directory;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod read_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod registration;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod storage_status;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod watch_directory;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod write_file;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod write_objects;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod write_records;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod write_schema;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod write_streams;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod advance_projection;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod compare_exchange;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod inspect_projection;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod read_projection;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod snapshot;

/* === Re-Exports === */

pub(crate) use contracts::contract;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use client::{
    AppendFileBytes, Capabilities, CreateFileProduction, CreateFileWriter, CreateSnapshot, DeleteFileProduction, DeleteSnapshot,
    GetFileBytes, GetFileProduction, GetIndexDefinition, GetProjectionDefinition, ListColumnRows, ListIndexEntries, ListRows,
    PublishFileProduction, SetIndexDefinition, SetProjectionDefinition,
};

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use advance_projection::AdvanceProjection;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use compare_exchange::CompareExchangeObject;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use inspect_projection::InspectProjection;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use read_projection::ReadProjection;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use snapshot::SavedSnapshots;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use capture_file::CaptureFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use delete_file::DeleteFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use follow_file::FollowFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use follow_objects::FollowObjects;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use follow_production::FollowProduction;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use follow_records::FollowRecords;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use follow_stream::FollowStream;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use hash_file::HashFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use list_directory::ListDirectory;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use read_file::ReadFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use registration::storage_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use storage_status::StorageStatus;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use watch_directory::WatchDirectory;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use write_file::WriteFile;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use write_objects::WriteObjects;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use write_records::WriteRecords;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use write_schema::WriteSchema;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use write_streams::WriteStreams;

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
fn output_timeout() -> crate::Error {
    harmos_storage::Error::backend(
        "storage output",
        harmos_storage::BackendFailure::Transient,
        std::io::Error::new(std::io::ErrorKind::TimedOut, "consumer delivery deadline exceeded"),
    )
    .into()
}

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
