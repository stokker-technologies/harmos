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

pub mod aggregate;
pub mod arrow;
mod backend;
mod batch;
mod batching;
mod capacity;
pub mod config;
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
pub mod json;
mod local;
mod manager;
mod object_admissions;
mod objects;
mod opened_capture;
pub mod query;
mod queue;
mod read_batch;
mod receipts;
mod schema_encoding;
mod selection;
pub mod signal;
mod snapshot;
mod staging;
pub mod state;
mod worker;
pub mod writes;

/* === Re-Exports === */

pub use backend::{BackendSnapshot, ReadLimits, ScanPage, StorageBackend, StorageBackends, StorageFuture};
pub use batch::{FileChunk, StorageBatch};
pub use config::{StorageFilesystemSettings, StorageRequestSettings, StorageResponseSettings, StorageSettings};
pub use directory::{DirectoryReader, DirectoryWatcher};
pub use error::{BackendFailure, Error, Result};
pub use file_batch::{FileReadBatch, FileReadEvent, FileReadSlice};
pub use file_capture::FileCapture;
pub use file_checkpoint::FileCheckpoint;
pub use file_follow::{FileFollow, FileFollower};
pub use file_production::ProductionFollower;
pub use file_reads::{FileRead, FileReader};
pub use frozen_file::FrozenFile;
pub use harmos_domain::model::storage::files;
pub use harmos_domain::model::storage::{
    ByteRange, CaptureConsistency, FileBatch, FileCompletion, FileEvent, FileHash, FileMetadata, FilePhase, FileProducer, FileProgress,
    FileReference, FileSession, FileSource, FollowStart, IdentityKind, IdentitySelection, MutationId, MutationWindow, ObjectCatalog,
    ObjectDefinition, ObjectDefinitions, ObjectEntry, ObjectKey, ObjectPoint, ObjectReceipt, ObjectRetention, ObjectRevision, ObjectValue,
    ObjectVersion, ObjectVersionToken, PayloadEncoding, RecordInput, RecordWrite, RecoveryPosition, SchemaInfo, SchemaInput, SchemaKey,
    StagedFile, StorageCursor, StorageInfo, StoreHistory, StoredIdentity, StoredObject, StoredRecord, StoredRow, StreamInput, StreamWrite,
};
pub use json::{JsonStorage, JsonStorageSettings};
pub use manager::{ErasedCatalog, StorageManager, StorageMetrics, WorkerOwner, encoded_size};
pub use objects::ObjectGroup;
pub use opened_capture::OpenedCapture;
pub use query::{ObjectRead, ObjectReader, Query, QueryReader, RecordRead, RecordReader, StreamRead, StreamReader};
pub use read_batch::{ReadBatch, ReadPages};
pub use receipts::{MutationReceipt, MutationValue};
pub use staging::StorageFile;

pub use harmos_domain::{Aggregate, Identifier, MemorySize, Message, btree_map_memory, schema};
pub use staging::checkpoint::FileSave;
pub use worker::Worker;
pub use writes::StorageSchema;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
#[cfg(test)]
extern crate harmos_domain as harmos;
extern crate self as harmos_storage;

// Local Imports
// Internal Paths Refer To This Crate's One Canonical Engine Namespace.
use crate as storage;

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
