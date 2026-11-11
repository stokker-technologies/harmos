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

#[cfg(feature = "runtime")]
mod abandon_file;
#[cfg(feature = "runtime")]
mod append_file;
#[cfg(feature = "runtime")]
mod capture_file;
#[cfg(feature = "runtime")]
mod create_file;
#[cfg(feature = "runtime")]
mod delete_file;
#[cfg(feature = "runtime")]
pub(crate) mod exchange;
#[cfg(feature = "runtime")]
mod finish_file;
#[cfg(feature = "runtime")]
mod follow_file;
#[cfg(feature = "runtime")]
mod follow_objects;
#[cfg(feature = "runtime")]
mod follow_production;
#[cfg(feature = "runtime")]
mod follow_records;
#[cfg(feature = "runtime")]
mod follow_stream;
mod hash_file;
#[cfg(feature = "runtime")]
mod list_directory;
#[cfg(feature = "runtime")]
mod query;
#[cfg(feature = "runtime")]
mod read_file;
#[cfg(feature = "runtime")]
mod registration;
#[cfg(feature = "runtime")]
mod resume_file;
#[cfg(feature = "runtime")]
mod storage_status;
#[cfg(feature = "runtime")]
mod watch_directory;
#[cfg(feature = "runtime")]
mod write_file;
#[cfg(feature = "runtime")]
mod write_objects;
#[cfg(feature = "runtime")]
mod write_records;
#[cfg(feature = "runtime")]
mod write_schema;
#[cfg(feature = "runtime")]
mod write_streams;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use abandon_file::AbandonFile;
#[cfg(feature = "runtime")]
pub use append_file::AppendFile;
#[cfg(feature = "runtime")]
pub use capture_file::CaptureFile;
#[cfg(feature = "runtime")]
pub use create_file::CreateFile;
#[cfg(feature = "runtime")]
pub use delete_file::DeleteFile;
#[cfg(feature = "runtime")]
pub use finish_file::FinishFile;
#[cfg(feature = "runtime")]
pub use follow_file::FollowFile;
#[cfg(feature = "runtime")]
pub use follow_objects::FollowObjects;
#[cfg(feature = "runtime")]
pub use follow_production::FollowProduction;
#[cfg(feature = "runtime")]
pub use follow_records::FollowRecords;
#[cfg(feature = "runtime")]
pub use follow_stream::FollowStream;
pub use hash_file::HashFile;
#[cfg(feature = "runtime")]
pub use list_directory::ListDirectory;
#[cfg(feature = "runtime")]
pub use query::Query;
#[cfg(feature = "runtime")]
pub use read_file::ReadFile;
#[cfg(feature = "runtime")]
pub use registration::Storage;
#[cfg(feature = "runtime")]
pub use resume_file::ResumeFile;
#[cfg(feature = "runtime")]
pub use storage_status::StorageStatus;
#[cfg(feature = "runtime")]
pub use watch_directory::WatchDirectory;
#[cfg(feature = "runtime")]
pub use write_file::WriteFile;
#[cfg(feature = "runtime")]
pub use write_objects::WriteObjects;
#[cfg(feature = "runtime")]
pub use write_records::WriteRecords;
#[cfg(feature = "runtime")]
pub use write_schema::WriteSchema;
#[cfg(feature = "runtime")]
pub use write_streams::WriteStreams;

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
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::OnTransient {
        max_retries: 3,
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
    timeout: harmos_domain::operations::UNBOUNDED,
    attempt_timeout: harmos_domain::operations::UNBOUNDED,
    cancellation_timeout: std::time::Duration::ZERO,
    retries: RetryPolicy::Never,
};

/* === Macros === */

#[harmos::catalog(harmos_path = crate)]
enum Legacy {
    HashFile(HashFile),
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
