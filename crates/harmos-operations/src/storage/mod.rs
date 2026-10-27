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

mod abandon_file;
mod append_file;
mod capture_file;
mod create_file;
mod delete_file;
#[cfg(feature = "runtime")]
pub(crate) mod exchange;
mod finish_file;
mod follow_file;
mod follow_objects;
mod follow_production;
mod follow_records;
mod follow_stream;
mod hash_file;
mod list_directory;
mod query;
mod read_file;
mod resume_file;
mod storage_status;
mod watch_directory;
mod write_file;
mod write_objects;
mod write_records;
mod write_schema;
mod write_streams;

/* === Re-Exports === */

pub use abandon_file::{AbandonFile, AbandonFileRequest};
pub use append_file::{AppendFile, AppendFileRequest};
pub use capture_file::{CaptureFile, CaptureFileRequest};
pub use create_file::{CreateFile, CreateFileRequest};
pub use delete_file::{DeleteFile, DeleteFileRequest};
pub use finish_file::{FinishFile, FinishFileRequest};
pub use follow_file::{FollowFile, FollowFileRequest};
pub use follow_objects::{FollowObjects, FollowObjectsRequest};
pub use follow_production::{FollowProduction, FollowProductionRequest};
pub use follow_records::{FollowBoundaries, FollowRecords, FollowRecordsRequest, Followed};
pub use follow_stream::{FollowStream, FollowStreamRequest};
pub use hash_file::{HashFile, HashFileRequest};
pub use list_directory::{ListDirectory, ListDirectoryRequest};
pub use query::{Query, QueryRequest};
pub use read_file::{ReadFile, ReadFileRequest};
pub use resume_file::{ResumeFile, ResumeFileRequest};
pub use storage_status::{StorageStatus, StorageStatusRequest};
pub use watch_directory::{WatchDirectory, WatchDirectoryRequest};
pub use write_file::{WriteFile, WriteFileRequest};
pub use write_objects::{WriteObjects, WriteObjectsRequest};
pub use write_records::{WriteRecords, WriteRecordsRequest};
pub use write_schema::{WriteSchema, WriteSchemaRequest};
pub use write_streams::{WriteStreams, WriteStreamsRequest};

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports

use harmos_domain::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};

/* === Constants === */

pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(60),
    attempt_timeout: Duration::from_secs(15),
    retries: RetryPolicy::OnTransient {
        max_retries: 3,
        backoff: Backoff::Exponential {
            initial: Duration::from_millis(250),
            maximum: Duration::from_secs(2),
            jitter: Jitter::Deterministic,
        },
    },
};

pub const EVERY_SCHEMA: &str = "*";

pub const STREAM_POLICY: OperationPolicy = OperationPolicy { attempt_timeout: POLICY.timeout, retries: RetryPolicy::Never, ..POLICY };

pub const IO_POLICY: OperationPolicy = OperationPolicy {
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    retries: RetryPolicy::Never,
};

/* === Macros === */

#[harmos::catalog(harmos_path = crate)]
pub enum Storage {
    ListDirectory(ListDirectory),
    WatchDirectory(WatchDirectory),
    Query(Query),
    ReadFile(ReadFile),
    HashFile(HashFile),
    CaptureFile(CaptureFile),
    FollowProduction(FollowProduction),
    FollowRecords(FollowRecords),
    FollowStream(FollowStream),
    FollowFile(FollowFile),
    StorageStatus(StorageStatus),
    ResumeFile(ResumeFile),
    CreateFile(CreateFile),
    AppendFile(AppendFile),
    FinishFile(FinishFile),
    AbandonFile(AbandonFile),
    WriteRecords(WriteRecords),
    WriteStreams(WriteStreams),
    WriteFile(WriteFile),
    DeleteFile(DeleteFile),
    WriteSchema(WriteSchema),
    WriteObjects(WriteObjects),
    FollowObjects(FollowObjects),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(feature = "runtime")]
const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(feature = "runtime")]
fn output_timeout() -> crate::Error {
    harmos_storage::Error::backend(
        "storage output",
        harmos_storage::BackendFailure::Transient,
        std::io::Error::new(std::io::ErrorKind::TimedOut, "consumer delivery deadline exceeded"),
    )
    .into()
}

#[cfg(feature = "runtime")]
fn classify(error: &crate::Error) -> harmos_domain::operations::FailureKind {
    match error {
        crate::Error::Storage(source) if source.backend_failure() == Some(harmos_storage::BackendFailure::Transient) => {
            harmos_domain::operations::FailureKind::Transient
        }
        _ => harmos_domain::operations::FailureKind::Permanent,
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use super::classify;
    use harmos_domain::operations::FailureKind;
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
