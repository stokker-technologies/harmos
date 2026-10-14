/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod abandon_file;
mod acknowledge;
mod append_file;
mod create_file;
mod data_selection;
mod delete_file;
mod file_mutation;
mod file_selection;
mod finish_file;
mod follow;
mod follow_file;
mod follow_objects;
mod follow_stream;
mod object_result;
mod object_selection;
mod query_files;
mod query_schemas;
mod query_storage;
mod query_stream;
mod read_files;
mod read_objects;
mod read_records;
mod reductions;
mod resume_file;
mod save;
mod selection;
mod write_objects;
mod write_records;
mod write_schema;
mod write_stream;

/* === Re-Exports === */
pub use abandon_file::AbandonFile;
pub use acknowledge::{Acknowledge, AcknowledgeRequest};
pub use append_file::{AppendFile, AppendFileRequest};
pub use create_file::{CreateFile, CreateFileRequest};
pub use data_selection::DataSelection;
pub use delete_file::{DeleteFile, DeleteFileRequest};
pub use file_mutation::FileMutation;
pub use file_selection::{FileBytes, FileSelection};
pub use finish_file::FinishFile;
pub use follow::{Follow, FollowBoundaries, Followed};
pub use follow_file::FollowFile;
pub use follow_objects::FollowObjects;
pub use follow_stream::FollowStream;
pub use object_result::ObjectResult;
pub use object_selection::ObjectSelection;
pub use query_files::QueryFiles;
pub use query_schemas::{QuerySchemas, QuerySchemasRequest};
pub use query_storage::{QueryStorage, QueryStorageRequest};
pub use query_stream::QueryStream;
pub use read_files::ReadFiles;
pub use read_objects::{ReadObjects, ReadObjectsRequest};
pub use read_records::ReadRecords;
pub use reductions::{Reductions, ReductionsRequest};
pub use resume_file::ResumeFile;
pub use save::{Save, SaveRequest};
pub use selection::Selection;
pub use write_objects::{WriteObjects, WriteObjectsRequest, WriteObjectsResponse};
pub use write_records::{WriteRecords, WriteRecordsRequest};
pub use write_schema::{WriteSchema, WriteSchemaRequest};
pub use write_stream::{WriteStream, WriteStreamRequest};

/* === Imports === */

// Standard Library Imports

use std::sync::Arc;
use std::time::Duration;

// Third Party Imports

// Local Imports
use crate::catalog::{Catalog, Contains, Member, Visitor};

use crate::operations::OperationManager;
use crate::operations::{Backoff, Jitter, OperationPolicy, RestartPolicy, RetryPolicy};

use crate::storage::StorageManager;

use crate::{Result, operations::Lane};

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

const STREAM_POLICY: OperationPolicy = OperationPolicy {
    attempt_timeout: POLICY.timeout,
    retries: RetryPolicy::Never,
    ..POLICY
};

/* === Macros === */
// Declare The Portable Catalog And Its Original Provider Bindings Together.
macro_rules! catalog {
    ($($leaf:ident => $module:ident),* $(,)?) => {
                pub enum Operations { $($leaf($leaf)),* }
        impl<V: Visitor $(+ Member<$leaf>)*> Catalog<V> for Operations {
            type Entries = [V::Entry; [$(stringify!($leaf)),*].len()];
            const ENTRIES: Self::Entries = [$(<V as Member<$leaf>>::ENTRY),*];
        }
        $(impl Contains<$leaf> for Operations {})*

        impl<C: 'static, V> Catalog<V> for Provider<C>
        where V: Visitor $(+ Member<$module::Handler<C>>)* {
            type Entries = [V::Entry; [$(stringify!($leaf)),*].len()];
            const ENTRIES: Self::Entries = [$(<V as Member<$module::Handler<C>>>::ENTRY),*];
        }
    };
}

catalog! {
    Save => save,
    QueryFiles => query_files,
    ReadRecords => read_records,
    QueryStream => query_stream,
    Follow => follow,
    FollowStream => follow_stream,
    ReadFiles => read_files,
    FollowFile => follow_file,
    QueryStorage => query_storage,
    ResumeFile => resume_file,
    CreateFile => create_file,
    AppendFile => append_file,
    FinishFile => finish_file,
    AbandonFile => abandon_file,
    WriteRecords => write_records,
    WriteStream => write_stream,
    DeleteFile => delete_file,
    WriteSchema => write_schema,
    QuerySchemas => query_schemas,
    WriteObjects => write_objects,
    ReadObjects => read_objects,
    FollowObjects => follow_objects,
    Acknowledge => acknowledge,
    Reductions => reductions,
}

/* === Type Aliases === */

/* === Structs === */

struct Provider<C>(std::marker::PhantomData<fn() -> C>);

/* === Enums === */
/* === Traits === */

/* === Implementations === */

impl Operations {
    pub fn register<C: 'static>(
        operations: &OperationManager,
        owner: &str,
        lane: impl Lane,
        storage: Arc<StorageManager<C>>,
    ) -> Result<()> {
        operations.register_wire_operations::<Provider<C>, _>(owner, lane, storage)
    }
}

/* === Public Functions === */

/* === Private Functions === */
const fn grpc(method: &'static str) -> crate::operations::Grpc {
    crate::operations::Grpc::new("harmos.storage.v1", "Storage", method)
}

fn survey(selector: &crate::Identifier) -> harmos_contract::plugin::Grant {
    harmos_contract::plugin::Grant::Survey(selector.as_str().to_owned().into())
}

fn classify(error: &crate::Error) -> crate::operations::Failure {
    let mut cause = error;
    while let crate::Error::Persistence { source } = cause {
        cause = source;
    }
    match cause {
        crate::Error::Backend {
            kind: crate::BackendFailure::Transient,
            ..
        } => crate::operations::Failure::Transient,
        _ => crate::operations::Failure::Permanent,
    }
}

/* === Tests === */
