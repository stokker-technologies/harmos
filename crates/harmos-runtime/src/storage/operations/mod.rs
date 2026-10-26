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

pub use harmos_domain::operations::storage::*;

/* === Imports === */

// Standard Library Imports

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::storage::StorageManager;

use crate::Result;

/* === Constants === */

const IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// Bind Each Declaration To Its Storage Implementation.
pub fn registration<C: 'static>(storage: Arc<StorageManager<C>>) -> Result<harmos_sdk::RegistrationBatch> {
    let mut dependencies = harmos_sdk::Dependencies::new();
    dependencies.insert(storage)?;
    let batch = harmos_sdk::RegistrationBatch::new(dependencies)
        .implementation::<AbandonFile, abandon_file::Handler<C>>()
        .implementation::<AppendFile, append_file::Handler<C>>()
        .implementation::<CaptureFile, capture_file::Handler<C>>()
        .implementation::<FinishFile, finish_file::Handler<C>>()
        .implementation::<CreateFile, create_file::Handler<C>>()
        .implementation::<DeleteFile, delete_file::Handler<C>>()
        .implementation::<FollowProduction, follow_production::Handler<C>>()
        .implementation::<FollowFile, follow_file::Handler<C>>()
        .implementation::<FollowObjects, follow_objects::Handler<C>>()
        .implementation::<FollowStream, follow_stream::Handler<C>>()
        .implementation::<FollowRecords, follow_records::Handler<C>>()
        .implementation::<ListDirectory, list_directory::Handler<C>>()
        .implementation::<HashFile, hash_file::Handler<C>>()
        .implementation::<Query, query::Handler<C>>()
        .implementation::<ReadFile, read_file::Handler<C>>()
        .implementation::<ResumeFile, resume_file::Handler<C>>()
        .implementation::<StorageStatus, storage_status::Handler<C>>()
        .implementation::<WatchDirectory, watch_directory::Handler<C>>()
        .implementation::<WriteFile, write_file::Handler<C>>()
        .implementation::<WriteObjects, write_objects::Handler<C>>()
        .implementation::<WriteRecords, write_records::Handler<C>>()
        .implementation::<WriteStreams, write_streams::Handler<C>>()
        .implementation::<WriteSchema, write_schema::Handler<C>>();
    Ok(batch)
}

/* === Private Functions === */

fn output_timeout() -> crate::Error {
    crate::Error::backend(
        "storage output",
        crate::BackendFailure::Transient,
        std::io::Error::new(std::io::ErrorKind::TimedOut, "consumer delivery deadline exceeded"),
    )
}

fn classify(error: &crate::Error) -> crate::operations::FailureKind {
    let mut cause = error;
    while let crate::Error::Persistence { source } = cause {
        cause = source;
    }
    match cause {
        crate::Error::Backend { kind: crate::BackendFailure::Transient, .. } => crate::operations::FailureKind::Transient,
        _ => crate::operations::FailureKind::Permanent,
    }
}

/* === Tests === */
