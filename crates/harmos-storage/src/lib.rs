/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[cfg(test)]
mod arrow_tests;
#[cfg(test)]
mod policy_tests;
mod admissions;
#[doc(hidden)]
pub mod aggregate;
pub mod arrow;
mod backend;
mod batch;
mod batching;
mod capacity;
pub mod config;
mod deletion;
mod error;
pub mod execution;
mod file_checkpoint;
mod file_follow;
mod file_reads;
pub mod follow;
mod manager;
mod memory;
mod object_admissions;
mod objects;
pub mod query;
mod queue;
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
pub use backend::{
    BackendSnapshot, ReadLimits, ScanPage, StorageBackend, StorageBackends, StorageFuture,
};
pub use batch::{StorageBatch, StorageEntry, StoragePart};
pub use config::{StorageBudget, StorageRequestSettings, StorageResponseSettings, StorageSettings};
pub use error::{BackendFailure, Error, Result};
pub use file_checkpoint::FileCheckpoint;
pub use file_follow::FileFollower;
pub use file_reads::FileReader;
pub use harmos_domain::model::storage::files;
pub use harmos_domain::model::storage::{
    ByteRange, FileBatch, FileCompletion, FileMetadata, FilePhase, FileProducer, FileProgress,
    FileReference, FileSession, MutationId, MutationWindow, ObjectCatalog, ObjectDefinition,
    ObjectDefinitions, ObjectEntry, ObjectKey, ObjectPoint, ObjectReceipt, ObjectRetention,
    ObjectRevision, ObjectValue, ObjectVersion, ObjectVersionToken, PayloadEncoding, RecordInput,
    RecordWrite, RecoveryPosition, SchemaInfo, SchemaInput, SchemaKey, StagedFile, StorageCursor,
    StorageInfo, StoreHistory, StoredObject, StoredRecord, StoredRow, StreamInput, StreamWrite,
};
pub use manager::{ErasedCatalog, StorageManager, StorageMetrics, WorkerOwner, encoded_size};
pub use objects::ObjectGroup;
pub use query::{ObjectRead, ObjectReader, QueryReader, RecordRead, RecordReader};
pub use receipts::{MutationReceipt, MutationValue};
pub use staging::StorageFile;
#[doc(hidden)]
pub use staging::checkpoint::FileSave;
pub use worker::Worker;
pub use writes::StorageSchema;

// Internal Paths Refer To This Crate's One Canonical Engine Namespace.
use crate as storage;
extern crate self as harmos_storage;
#[cfg(feature = "grpc")]
pub use harmos_domain::Protobuf;
pub use harmos_domain::{Aggregate, Identifier, MemorySize, Message, btree_map_memory, schema};
