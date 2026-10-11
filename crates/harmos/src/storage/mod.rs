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
mod bootstrap;
mod browse;
pub(crate) mod capture;
mod config;
mod deletion;
mod file_checkpoint;
mod file_follow;
mod file_reads;

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
#[path = "../series/mod.rs"]
pub mod series;

/* === Re-Exports === */
use crate::object_catalog::{ObjectCatalog, ObjectValue, SchemaKey, StoredObject};
use crate::storage_data::{StoredRecord, StoredRow};
use crate::storage_files::{
    FileMetadata, FilePhase, FileProgress, FileReference, FileSession, StagedFile,
};
use crate::storage_mutations::{FileProducer, MutationId, MutationWindow};
use crate::storage_object_fields::{
    ObjectComparison, ObjectMetadata, ObjectNullOrder, ObjectScalar,
};
use crate::storage_object_values::{
    ObjectEntry, ObjectPoint, ObjectReceipt, ObjectRevision, ObjectVersion, ObjectVersionToken,
    ObjectVersions,
};
use crate::storage_position::{RecoveryPosition, StorageCursor, StoreHistory};
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
