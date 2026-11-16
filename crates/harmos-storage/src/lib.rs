/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod admissions;

mod backend;
mod batch;
mod batching;
#[cfg(feature = "testing")]
pub mod bench_support;
mod capacity;
mod compound;
pub mod config;
pub mod conformance;
mod content;
mod definitions;
mod deletion;
pub mod directory;
mod error;
pub mod execution;
mod file_batch;
mod file_capture;
mod file_checkpoint;
mod file_follow;
mod file_hash;
mod file_production;
mod file_reads;
pub mod follow;
mod frozen_file;
mod local;
mod manager;
pub mod memory;
mod object_admissions;
mod objects;
mod opened_capture;
mod persistence_work;
pub mod query;
mod queue;

mod receipts;
mod record_view;

mod schema_sets;
mod selection;
mod snapshot;
mod staging;
pub mod state;

mod worker;
pub mod writes;

/* === Re-Exports === */

pub use backend::{BackendSnapshot, ReadLimits, ScanPage, StorageBackend, StorageBackends, StorageFuture, StorageWork};
pub use batch::{FileChunk, StorageBatch};
pub use compound::{CompoundExchange, CompoundWrite};
pub use config::{StorageFilesystemSettings, StoragePolicy, StorageRequestSettings, StorageResponseSettings, StorageSettings};
pub use content::{ContentLease, ContentRead, ContentSource};
pub use definitions::{DefinitionInput, DefinitionKind, DefinitionOutcome, DefinitionRefusal};
pub use directory::{DirectoryReader, DirectoryWatcher};
pub use error::{BackendFailure, Error, Result};
pub use file_batch::{FileReadBatch, FileReadEvent, FileReadSlice};
pub use file_capture::FileCapture;
pub use file_checkpoint::FileCheckpoint;
pub use file_follow::{FileFollow, FileFollower};
pub use file_production::ProductionFollower;
pub use file_reads::{ContentRange, FileBytesRange, FileRead, FileReader};
pub use frozen_file::FrozenFile;
pub use harmos_domain::model::storage::files;
pub use harmos_domain::model::storage::{
    ByteRange, CaptureConsistency, CompoundOutcome, CompoundReceipt, FileBatch, FileCompletion, FileEvent, FileHash, FileMetadata,
    FilePhase, FileProducer, FileProgress, FileReference, FileSession, FileSource, FollowStart, IdentityKind, IdentitySelection,
    MutationId, MutationItem, MutationWindow, ObjectCatalog, ObjectDefinition, ObjectEntry, ObjectHandle, ObjectKey, ObjectPoint,
    ObjectReceipt, ObjectRetention, ObjectRevision, ObjectValue, ObjectVersion, ObjectVersionToken, PayloadEncoding, RecordInput,
    RecordWrite, RecoveryPosition, SchemaKey, StagedFile, StorageCursor, StorageInfo, StoreHistory, StoredIdentity, StoredObject,
    StoredRecord, StoredRow, StreamInput, StreamWrite,
};
pub use manager::{StorageManager, StorageMetrics, WorkerOwner, encoded_size};
pub use objects::{CompareExchange, ObjectExchange, ObjectGroup};
pub use opened_capture::OpenedCapture;
pub use persistence_work::{PersistenceWork, StageWork};

pub use harmos_domain::model::storage::{ReadBatch, ReadPages};
pub use query::QueryReader;
pub use receipts::{MutationHeader, MutationReceipt, MutationValue};
pub use record_view::RecordView;
pub use schema_sets::{SchemaExchange, SchemaWrite, SchemaWriteReceipt};
pub use snapshot::Snapshot;
pub use staging::{FileProductionStatus, StorageFile};

pub use harmos_domain::model::storage::StorageSchema;
pub use harmos_domain::{Aggregate, Identifier, MemorySize, Message, btree_map_memory, schema};
pub use staging::checkpoint::FileSave;
pub use worker::Worker;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
#[cfg(test)]
extern crate harmos_domain as harmos;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
#[allow(dead_code, reason = "capacity invariants use the shared in-memory storage backends")]
mod test_backend {
    use crate::StorageFilesystemSettings;
    // Unit Consumers Own Their Explicit Fixture Budgets.
    pub use crate::memory::{Backend, RecoveryBackend};
    use crate::{Identifier, StorageRequestSettings, StorageResponseSettings, StorageSettings, StoreHistory};
    use std::num::NonZeroUsize;
    use std::time::Duration;

    pub fn history() -> StoreHistory {
        StoreHistory {
            store: Identifier::new("test-store").unwrap(),
            epoch: Identifier::new("test-epoch").unwrap(),
        }
    }

    pub fn storage_settings() -> StorageSettings {
        StorageSettings {
            transfer: Default::default(),
            policy: Default::default(),
            filesystem: StorageFilesystemSettings {
                poll_interval: Duration::from_millis(100),
                max_entries: NonZeroUsize::new(100_000).unwrap(),
                event_capacity: NonZeroUsize::new(1024).unwrap(),
            },

            request: StorageRequestSettings {
                max_file_productions: NonZeroUsize::new(256).unwrap(),
                max_mutation_receipts: NonZeroUsize::new(1024).unwrap(),
                max_pending_queries: nz(64),
                max_active_queries: nz(4),
                max_pending_write_batches: nz(256),
                max_active_file_operations: nz(1),
                max_pending_write_bytes: nz(16 * 1024 * 1024),
            },
            response: StorageResponseSettings {
                batch_rows: nz(64),
                batch_bytes: nz(64 * 1024),
            },
        }
    }

    fn nz(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).unwrap()
    }
}
