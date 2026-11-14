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
pub mod config;
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
#[cfg(feature = "sqlite")]
pub mod sqlite;
mod staging;
pub mod state;

mod worker;
pub mod writes;

/* === Re-Exports === */

pub use backend::{BackendSnapshot, ReadLimits, ScanPage, StorageBackend, StorageBackends, StorageFuture, StorageWork};
pub use batch::{FileChunk, StorageBatch};
pub use config::{
    StorageFilesystemSettings, StoragePolicy, StorageRequestSettings, StorageResponseSettings, StorageSettings,
};
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
    ByteRange, CaptureConsistency, FileBatch, FileCompletion, FileEvent, FileHash, FileMetadata, FilePhase, FileProducer, FileProgress,
    FileReference, FileSession, FileSource, FollowStart, IdentityKind, IdentitySelection, MutationId, MutationWindow, ObjectCatalog,
    ObjectDefinition, ObjectEntry, ObjectHandle, ObjectKey, ObjectPoint, ObjectReceipt, ObjectRetention, ObjectRevision, ObjectValue,
    ObjectVersion, ObjectVersionToken, PayloadEncoding, RecordInput, RecordWrite, RecoveryPosition, SchemaKey, StagedFile, StorageCursor,
    StorageInfo, StoreHistory, StoredIdentity, StoredObject, StoredRecord, StoredRow, StreamInput, StreamWrite,
};
pub use manager::{StorageManager, StorageMetrics, WorkerOwner, encoded_size};
pub use objects::{CompareExchange, ObjectExchange, ObjectGroup};
pub use opened_capture::OpenedCapture;
pub use persistence_work::{PersistenceWork, StageWork};

pub use harmos_domain::model::storage::{ReadBatch, ReadPages};
pub use receipts::{MutationHeader, MutationReceipt, MutationValue};
pub use record_view::RecordView;
pub use schema_sets::{SchemaExchange, SchemaWrite, SchemaWriteReceipt};
pub use snapshot::Snapshot;
pub use query::QueryReader;
pub use staging::{FileProductionStatus, StorageFile};

pub use harmos_domain::{Aggregate, Identifier, MemorySize, Message, btree_map_memory, schema};
pub use staging::checkpoint::FileSave;
pub use worker::Worker;
pub use harmos_domain::model::storage::StorageSchema;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
#[cfg(test)]
extern crate harmos_domain as harmos;
extern crate self as harmos_storage;

// Local Imports
// Internal Paths Refer To This Crate's One Canonical Engine Namespace.


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
#[path = "../tests/support/storage.rs"]
mod test_backend;
