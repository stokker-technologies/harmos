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
pub mod arrow;
pub(crate) mod attachments;
mod backend;
mod batch;
mod batching;
mod capacity;
pub(crate) mod capture;
pub(crate) mod catalog;
pub mod config;
pub(crate) mod consumer;
mod deletion;
mod file_checkpoint;
mod file_follow;
mod file_reads;
pub mod follow;
pub(crate) mod manager;
mod memory;
mod object_admissions;
mod objects;
pub mod operations;
pub mod query;
mod queue;
mod receipts;
mod saving;
mod schema;
mod selection;
mod snapshot;
mod staging;
pub mod state;
mod worker;
pub mod writes;

/* === Re-Exports === */
pub use crate::storage::catalog::{
    ObjectCatalog, ObjectDefinition, ObjectDefinitions, StoredObject,
};
pub use attachments::{Horizon, Payload, Recovery, Source};
pub use backend::{
    BackendSnapshot, ReadLimits, ScanPage, StorageBackend, StorageBackends, StorageFuture,
};
pub use batch::{StorageBatch, StorageEntry, StoragePart};
pub use capture::{ProjectedRecords, RecordProjection};
pub use config::{StorageBudget, StorageRequestSettings, StorageResponseSettings, StorageSettings};
pub use consumer::{Batch, Cursor, JournalConsumer, Series, StreamConsumer};
pub use file_checkpoint::FileCheckpoint;
pub use file_follow::FileFollower;
pub use file_reads::FileReader;
pub use harmos_domain::model::storage::files;
pub use harmos_domain::model::storage::{
    ByteRange, FileBatch, FileCompletion, FileMetadata, FilePhase, FileProducer, FileProgress,
    FileReference, FileSession, MutationId, MutationWindow, ObjectEntry, ObjectKey, ObjectPoint,
    ObjectReceipt, ObjectRetention, ObjectRevision, ObjectValue, ObjectVersion, ObjectVersionToken,
    PayloadEncoding, ReadFile, RecordInput, RecordWrite, RecoveryPosition, SchemaInfo, SchemaInput,
    SchemaKey, StagedFile, StorageCursor, StorageInfo, StoreHistory, StoredRecord, StoredRow,
    StreamInput, StreamWrite,
};
pub use manager::{ErasedCatalog, StorageManager, StorageStatus};
pub use objects::ObjectGroup;
pub use query::{ObjectRead, ObjectReader, QueryReader, RecordRead, RecordReader};
pub use receipts::{MutationReceipt, MutationValue};
pub use staging::StorageFile;
pub(crate) use staging::checkpoint::FileSave;
pub use writes::StorageSchema;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

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
