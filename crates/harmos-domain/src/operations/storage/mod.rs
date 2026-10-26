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

pub use abandon_file::AbandonFile;
pub use append_file::AppendFile;
pub use capture_file::CaptureFile;
pub use create_file::CreateFile;
pub use delete_file::DeleteFile;
pub use finish_file::FinishFile;
pub use follow_file::FollowFile;
pub use follow_objects::FollowObjects;
pub use follow_production::FollowProduction;
pub use follow_records::{FollowBoundaries, FollowRecords, Followed};
pub use follow_stream::FollowStream;
pub use hash_file::HashFile;
pub use list_directory::ListDirectory;
pub use query::Query;
pub use read_file::ReadFile;
pub use resume_file::ResumeFile;
pub use storage_status::StorageStatus;
pub use watch_directory::WatchDirectory;
pub use write_file::WriteFile;
pub use write_objects::WriteObjects;
pub use write_records::WriteRecords;
pub use write_schema::WriteSchema;
pub use write_streams::WriteStreams;

/* === Imports === */

// Standard Library Imports
use std::time::Duration;
// Third Party Imports

// Local Imports
use crate::catalog::{Catalog, Contains, Member, Visitor};
use crate::operations::{Backoff, Jitter, OperationPolicy, RestartPolicy, RetryPolicy};

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
    restart: RestartPolicy::Never,
};

pub const EVERY_SCHEMA: &str = "*";

pub const STREAM_POLICY: OperationPolicy = OperationPolicy { attempt_timeout: POLICY.timeout, retries: RetryPolicy::Never, ..POLICY };

pub const IO_POLICY: OperationPolicy = OperationPolicy {
    timeout: crate::operations::UNBOUNDED,
    attempt_timeout: crate::operations::UNBOUNDED,
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

macro_rules! catalog {
    ($($leaf:ident),* $(,)?) => {
        pub enum Operations { $($leaf($leaf)),* }
        impl<V: Visitor $(+ Member<$leaf>)*> Catalog<V> for Operations {
            type Entries = [V::Entry; [$(stringify!($leaf)),*].len()];
            const ENTRIES: Self::Entries = [$(<V as Member<$leaf>>::ENTRY),*];
        }
        $(impl Contains<$leaf> for Operations {})*
    };
}

catalog! {
    ListDirectory,
    WatchDirectory,
    Query,
    ReadFile,
    HashFile,
    CaptureFile,
    FollowProduction,
    FollowRecords,
    FollowStream,
    FollowFile,
    StorageStatus,
    ResumeFile,
    CreateFile,
    AppendFile,
    FinishFile,
    AbandonFile,
    WriteRecords,
    WriteStreams,
    WriteFile,
    DeleteFile,
    WriteSchema,
    WriteObjects,
    FollowObjects,
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
