/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Backend Contract

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::query::{FileSelection, ObjectSelection, RecordSelection, SchemaSelection, StreamSelection};
use super::{
    FileCheckpoint, FileChunk, FileMetadata, FileProgress, FileReference, MutationReceipt, ObjectCatalog, ObjectEntry, ObjectKey,
    ObjectPoint, SchemaKey, StagedFile, StorageBatch, StorageCursor, StorageSchema, StoreHistory, StoredRecord, StoredRow,
};
use crate::{Error, Result};
pub use harmos_domain::model::storage::{BackendSnapshot, ReadLimits};
use crate::execution::Execution;
use crate::memory::{Backend, Recovery};
use harmos_domain::model::storage::{IdentitySelection, StoredIdentity};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

/* === Structs === */

pub struct StorageBackends {
    pub primary: Arc<dyn StorageBackend>,
    pub state: Arc<dyn super::state::StateBackend>,
}



#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanPage {
    pub after: u64,
    pub rows: NonZeroUsize,
    pub bytes: NonZeroUsize,
}



/// Cumulative physical work on one backend owner; differences describe an isolated operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct StorageWork {
    pub synchronous_commits: u64,
    pub synchronous_commit_ns: u64,
    pub maximum_synchronous_commit_ns: u64,
    pub writing_ns: u64,
    pub maximum_writing_ns: u64,
    pub source_rows: u64,
    pub source_bytes: u64,
    pub decoded_chunks: u64,
    pub bucket_rows: u64,
    pub bucket_steps: u64,
    pub buckets: u64,
    pub index_entries: u64,
    pub index_steps: u64,
    pub index_prepares: u64,
}

/* === Enums === */

/* === Traits === */

pub trait StorageBackend: Send + Sync + 'static {
    /// Optional immutable record pages. Views retain their data and checked encoded size.
    fn list_record_views<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _selection: &'a RecordSelection, _page: ScanPage,
    ) -> StorageFuture<'a, Option<Vec<crate::RecordView>>> {
        Box::pin(async { Ok(None) })
    }

    /// Optionally locate the exclusive source position after skipping selected records.
    /// None retains sequential execution; an exhausted selection returns the saved cut.
    fn get_record_position<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _selection: &'a RecordSelection, _offset: u64,
    ) -> StorageFuture<'a, Option<u64>> {
        Box::pin(async { Ok(None) })
    }

    /// Optional work evidence. Backends without instrumentation return zero counters.
    fn get_work(&self) -> StorageFuture<'_, StorageWork> {
        Box::pin(async { Ok(StorageWork::default()) })
    }

    /// Advance natively when supported, or return None for the shared bounded executor.
    /// None Retains The Shared Executor; Expected Revision Is Checked Atomically.
    fn advance_projection<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _name: &'a crate::Identifier, _expected_revision: Option<u64>, _work: ReadLimits,
        _cancel: &'a harmos_query::columnar::Cancellation,
    ) -> StorageFuture<'a, Option<u64>> {
        Box::pin(async { Ok(None) })
    }

    fn set_object<'a>(&'a self, _exchange: &'a crate::ObjectExchange) -> StorageFuture<'a, crate::CompareExchange> {
        unsupported("conditional objects")
    }

    fn delete_object_receipts<'a>(&'a self, _through: &'a StorageCursor) -> StorageFuture<'a, ()> {
        unsupported("conditional object receipt retention")
    }

    fn set_projection_definition<'a>(
        &'a self, _definition: &'a crate::query::projection::ProjectionDefinition, _expected: Option<u64>, _rebuild: bool,
        _mutation: Option<&'a crate::MutationHeader>,
    ) -> StorageFuture<'a, crate::DefinitionOutcome> {
        unsupported("projection registration")
    }

    /// Resolve The Current Definition Or An Explicit Retained Revision.
    fn get_projection_definition<'a>(
        &'a self, _name: &'a crate::Identifier, _revision: Option<u64>,
    ) -> StorageFuture<'a, (crate::query::projection::ProjectionDefinition, u64)> {
        unsupported("get projection definition")
    }

    /// None Selects The Newest Readable Durable Frontier For This Revision.
    fn get_projection_status<'a>(
        &'a self, _name: &'a crate::Identifier, _revision: u64, _frontier: Option<&'a StorageCursor>,
    ) -> StorageFuture<'a, Option<crate::query::projection::ProjectionStatus>> {
        unsupported("get projection status")
    }

    fn set_projection_output(&self, _publication: crate::query::projection::ProjectionPublication) -> StorageFuture<'_, ()> {
        unsupported("projection publication")
    }

    fn list_projection_rows<'a>(
        &'a self, _read: &'a crate::query::projection::ProjectionRead,
    ) -> StorageFuture<'a, crate::query::projection::ProjectionPage> {
        unsupported("projection read")
    }

    fn delete_projection_history<'a>(
        &'a self, _name: &'a crate::Identifier, _revision: u64, _through: &'a StorageCursor,
    ) -> StorageFuture<'a, ()> {
        unsupported("projection retention")
    }

    fn set_index_definition<'a>(
        &'a self, _definition: &'a harmos_query::index::IndexDefinition, _expected: Option<u64>, _rebuild: bool, _work: ReadLimits,
        _cancel: &'a harmos_query::columnar::Cancellation, _mutation: Option<&'a crate::MutationHeader>,
    ) -> StorageFuture<'a, crate::DefinitionOutcome> {
        unsupported("storage index registration")
    }

    fn get_index_definition<'a>(&'a self, name: &'a crate::Identifier) -> StorageFuture<'a, (harmos_query::index::IndexDefinition, u64)> {
        Box::pin(async move { Err(Error::IndexNotFound { name: name.clone() }) })
    }

    fn list_index_entries<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _read: &'a harmos_query::index::IndexRead,
        _cancel: &'a harmos_query::columnar::Cancellation,
    ) -> StorageFuture<'a, harmos_query::index::IndexPage> {
        unsupported("native index scan")
    }

    fn capabilities(&self) -> crate::query::columnar::StorageCapabilities {
        crate::query::columnar::StorageCapabilities::default()
    }

    fn list_column_rows<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _read: &'a harmos_query::columnar::ArrowScan,
        _cancel: &'a harmos_query::columnar::Cancellation,
    ) -> StorageFuture<'a, harmos_query::columnar::ArrowPage> {
        unsupported("native Arrow scan")
    }

    // Lifecycle
    fn get_history(&self) -> &StoreHistory;
    fn create_session(&self) -> StorageFuture<'_, StorageCursor>;
    fn get_saved_position(&self) -> StorageFuture<'_, StorageCursor>;
    fn delete_session(&self) -> StorageFuture<'_, ()>;

    // Independent Durable Writes; Replayed Positions Must Be Idempotent.
    fn append_schemas(&self, write: crate::SchemaWrite) -> StorageFuture<'_, crate::SchemaWriteReceipt>;
    fn append_object_revisions(&self, batch: StorageBatch<ObjectEntry>) -> StorageFuture<'_, StorageCursor>;
    fn append_records(&self, batch: StorageBatch<StoredRecord>) -> StorageFuture<'_, StorageCursor>;
    fn append_samples(&self, batch: StorageBatch<StoredRow>) -> StorageFuture<'_, StorageCursor>;
    fn append_file_references(&self, batch: StorageBatch<FileReference>) -> StorageFuture<'_, StorageCursor>;

    // Historical Reads
    fn create_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot>;
    fn delete_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()>;
    fn list_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>>;
    fn list_object_revisions<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>>;
    fn list_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>>;
    fn list_samples<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>>;
    fn list_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>>;
    fn get_object<'a>(&'a self, snapshot: &'a BackendSnapshot, point: &'a ObjectPoint) -> StorageFuture<'a, Option<ObjectEntry>>;
    fn get_file_bytes<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>>;

    // Object Lifecycle Retention
    fn delete_object_history<'a>(&'a self, schema: &'a SchemaKey, item: &'a ObjectKey, through: &'a StorageCursor)
    -> StorageFuture<'a, ()>;

    fn delete_expired_object_revisions<'a>(
        &'a self, through: &'a StorageCursor, max_entries: NonZeroUsize, retention: &'a ObjectCatalog,
    ) -> StorageFuture<'a, bool>;

    // Finished File Publication
    fn create_file_staging<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile>;
    fn append_file_bytes<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: FileChunk) -> StorageFuture<'a, ()>;
    fn publish_file_production<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference>;
    fn delete_file_staging<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()>;

    // Resumable File Sessions

    fn create_file_production<'a>(&'a self, _metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        unsupported("file session")
    }
    fn set_file_writer_generation<'a>(&'a self, _file: &'a StagedFile, _expected: u64, _next: u64) -> StorageFuture<'a, ()> {
        unsupported("file session")
    }
    fn set_file_production_checkpoints(&self, _checkpoint: StorageBatch<FileCheckpoint>) -> StorageFuture<'_, ()> {
        unsupported("file checkpoint")
    }
    fn list_file_production_checkpoints(&self, _limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        unsupported("file recovery")
    }
    fn get_file_production<'a>(&'a self, _file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        unsupported("file following")
    }
    fn get_file_production_bytes<'a>(
        &'a self, _file: &'a StagedFile, _offset: u64, _max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        unsupported("file following")
    }
    fn delete_file_production<'a>(&'a self, _staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        unsupported("file session")
    }

    fn delete_file<'a>(&'a self, _file: &'a StagedFile) -> StorageFuture<'a, ()> {
        unsupported("file deletion")
    }

    // Idempotent Mutation Receipts

    fn append_mutation_receipts(&self, _receipts: StorageBatch<MutationReceipt>) -> StorageFuture<'_, ()> {
        unsupported("storage mutations")
    }

    fn list_mutation_receipts(&self, _limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        unsupported("storage mutations")
    }

    // Stored Identity Catalog
    fn list_identities<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _selection: &'a IdentitySelection, _page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredIdentity>> {
        unsupported("storage identity catalog")
    }
}

/* === Implementations === */

impl StorageBackends {
    pub fn in_memory(catalog: ObjectCatalog, clock: Execution) -> Result<Self> {
        let mut identity = [0u8; 32];
        getrandom::fill(&mut identity).map_err(|source| Error::backend("create in-memory storage identity", crate::BackendFailure::Permanent, source))?;
        let store = identity[..16].iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let epoch = identity[16..].iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let history = StoreHistory { store: crate::Identifier::new(store).map_err(harmos_domain::Error::from)?, epoch: crate::Identifier::new(epoch).map_err(harmos_domain::Error::from)? };
        Ok(Self {
            primary: Arc::new(Backend::with_execution(catalog, history.clone(), clock)),
            state: Arc::new(Recovery::new(history)),
        })
    }
}



/* === Public Functions === */

/* === Private Functions === */

fn unsupported<'a, T>(field: &'static str) -> StorageFuture<'a, T> {
    Box::pin(async move {
        Err(Error::backend(
            field,
            crate::BackendFailure::Unsupported,
            std::io::Error::new(std::io::ErrorKind::Unsupported, "backend does not support this capability"),
        ))
    })
}

/* === Tests === */
