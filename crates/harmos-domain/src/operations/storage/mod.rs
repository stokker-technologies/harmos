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
mod create_file;
mod delete_file;
mod file_mutation;
mod finish_file;
mod follow;
mod follow_file;
mod follow_objects;
mod follow_stream;
mod query;
mod query_storage;
mod read_file;
mod resume_file;
mod write_file;
mod write_objects;
mod write_records;
mod write_schema;
mod write_stream;

/* === Re-Exports === */
pub use abandon_file::AbandonFile;
pub use append_file::{AppendFile, AppendFileRequest};
pub use create_file::{CreateFile, CreateFileRequest};
pub use delete_file::{DeleteFile, DeleteFileRequest};
pub use file_mutation::FileMutation;
pub use finish_file::FinishFile;
pub use follow::{Follow, FollowBoundaries, Followed};
pub use follow_file::FollowFile;
pub use follow_objects::FollowObjects;
pub use follow_stream::FollowStream;
pub use query::Query;
pub use query_storage::{QueryStorage, QueryStorageRequest};
pub use read_file::ReadFile;
pub use resume_file::ResumeFile;
pub use write_file::WriteFile;
pub use write_objects::WriteObjects;
pub use write_records::WriteRecords;
pub use write_schema::{WriteSchema, WriteSchemaRequest};
pub use write_stream::WriteStreams;

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

pub const STREAM_POLICY: OperationPolicy = OperationPolicy {
    attempt_timeout: POLICY.timeout,
    retries: RetryPolicy::Never,
    ..POLICY
};

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
    Query,
    ReadFile,
    Follow,
    FollowStream,
    FollowFile,
    QueryStorage,
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
const fn grpc(method: &'static str) -> crate::operations::Grpc {
    crate::operations::Grpc::new("harmos.storage.v1", "Storage", method)
}

/* === Tests === */
