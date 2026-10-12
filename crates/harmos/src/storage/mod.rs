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
pub(crate) mod attachments;
mod backend;
mod batch;
mod batching;
mod bootstrap;
mod browse;
pub(crate) mod capture;
pub(crate) mod catalog;
pub(crate) mod codec;
mod config;
pub(crate) mod consumer;
pub(crate) mod data;
mod deletion;
mod file_checkpoint;
mod file_follow;
mod file_reads;
pub(crate) mod files;
pub(crate) mod mutations;
pub(crate) mod object_fields;
pub(crate) mod object_values;
pub mod operations;
pub(crate) mod position;
pub(crate) mod read_selection;
pub(crate) mod schema_input;
pub(crate) mod timestamp;

mod follow;
pub mod frame;
pub(crate) mod manager;
mod object_admissions;
mod objects;
mod production;
mod query;
mod queue;
mod reads;
mod receipts;
pub mod recovery;
pub mod reduce;
mod replication;
mod saving;
mod schema;
mod selection;
mod staging;
mod transfer;
mod views;
mod work;
mod worker;
mod writes;

#[cfg(feature = "signal")]
pub mod series;

/* === Re-Exports === */
pub use crate::storage::catalog::{
    ObjectCatalog, ObjectDefinition, ObjectDefinitions, ObjectKey, ObjectRetention, ObjectValue,
    PayloadEncoding, SchemaKey, StoredObject,
};
pub use crate::storage::data::{RecordInput, StoredRecord, StoredRow, StreamInput};
pub use crate::storage::files::{
    FileMetadata, FilePhase, FileProgress, FileReference, FileSession, StagedFile,
};
pub use crate::storage::mutations::{FileProducer, MutationId, MutationWindow, StorageInfo};
pub use crate::storage::object_fields::{
    ObjectComparison, ObjectFields, ObjectFilter, ObjectMetadata, ObjectNullOrder, ObjectScalar,
    ObjectScalarKind, ObjectSort,
};
pub use crate::storage::object_values::{
    ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectVersion, ObjectVersionToken,
    ObjectVersions,
};
pub use crate::storage::position::{RecoveryPosition, StorageCursor, StoreHistory};
pub use attachments::{Horizon, Payload, Recovery, Source};
pub use backend::{StorageBackend, StorageBackends, StorageFuture};
pub use batch::{StorageBatch, StorageEntry, StoragePart};
pub use bootstrap::{
    BootstrapId, BootstrapManifest, CollectionDigest, RetainedBatch, RetainedCoverage,
    RetainedSnapshot,
};
pub use browse::{
    ObjectBrowseCursor, ObjectBrowsePage, ObjectBrowseQuery, ObjectBrowseRequest,
    ObjectBrowseResult, ObjectColumn, ObjectFieldsQuery, ObjectOrder, ObjectPredicate,
};
pub use capture::{ProjectedRecords, RecordProjection};
pub use config::StorageSettings;
pub use consumer::{Batch, Cursor, JournalConsumer, Series, StreamConsumer};
pub use file_checkpoint::FileCheckpoint;
pub use file_follow::FileFollower;
pub use file_reads::{FileChunk, StorageFileReader};
pub use follow::{QueryFollower, QueryReplay};
pub use frame::{Collected, Explain, Frame, FrameQuery, FrameResult};
pub use manager::{ErasedCatalog, RecordWrite, StorageManager, StorageStatus, StreamWrite};
pub use objects::ObjectGroup;
pub use production::{
    StorageProduction, StorageProductionComputation, StorageProductionPreparation,
};
pub use query::{
    EncodedObjectQuery, FileQuery, FileQueryResult, FollowQuery, ObjectQuery, ObjectQueryResult,
    Query, QueryLimits, QueryOptions, QueryPage, QueryResult, QueryValue, RecordQuery,
    RecordQueryResult, SchemaQuery, SchemaQueryResult, SharedQueryValue, StreamQuery,
    StreamQueryResult,
};
pub use reads::{
    BackendSnapshot, ReadCursor, ReadHistory, ReadKind, ReadLimits, ReadOrder, ReadPage,
    ReadSelection, ReadView, SourceTimeRange,
};
pub use receipts::{MutationReceipt, MutationValue};
pub use replication::{ReplicaHolder, ReplicaReceipt, ReplicaStatus, ReplicaTarget};
pub use schema::SchemaEncoding;
pub use schema_input::{SchemaInfo, SchemaInput};
pub use staging::StorageFile;
pub(crate) use staging::checkpoint::FileSave;
pub(crate) use views::{StoragePage, StorageReader};
pub use views::{StorageView, StorageViewComputation};
pub use work::{StorageReadWork, StorageReadWorkPreparation};
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
