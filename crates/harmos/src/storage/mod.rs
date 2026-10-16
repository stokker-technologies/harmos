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
pub(crate) mod codec;
pub mod config;
pub(crate) mod consumer;
pub(crate) mod data;
mod deletion;
mod file_checkpoint;
mod file_follow;
mod file_reads;
pub mod files;
pub mod follow;
pub(crate) mod manager;
mod memory;
pub(crate) mod mutations;
mod object_admissions;
pub(crate) mod object_values;
mod objects;
pub mod operations;
pub(crate) mod position;
pub mod query;
mod queue;
mod receipts;
mod saving;
mod schema;
pub(crate) mod schema_input;
mod selection;
mod snapshot;
mod staging;
pub mod state;
pub(crate) mod timestamp;
mod worker;
pub mod writes;

/* === Re-Exports === */
pub use crate::storage::catalog::{
    ObjectCatalog, ObjectDefinition, ObjectDefinitions, ObjectKey, ObjectRetention, ObjectValue,
    PayloadEncoding, SchemaKey, StoredObject,
};
pub use crate::storage::data::{RecordInput, StoredRecord, StoredRow, StreamInput};
pub use crate::storage::files::{
    ByteRange, FileBatch, FileCompletion, FileMetadata, FilePhase, FileProgress, FileReference,
    FileSession, ReadFile, StagedFile,
};
pub use crate::storage::mutations::{FileProducer, MutationId, MutationWindow, StorageInfo};
pub use crate::storage::object_values::{
    ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectVersion, ObjectVersionToken,
};
pub use crate::storage::position::{RecoveryPosition, StorageCursor, StoreHistory};
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
pub use manager::{ErasedCatalog, RecordWrite, StorageManager, StorageStatus, StreamWrite};
pub use objects::ObjectGroup;
pub use query::QueryReader;
pub use receipts::{MutationReceipt, MutationValue};
pub use schema_input::{SchemaInfo, SchemaInput};
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
