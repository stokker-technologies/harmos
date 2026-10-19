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
    FileCheckpoint, FileMetadata, FileProgress, FileReference, MutationReceipt, ObjectEntry, ObjectKey, ObjectPoint, SchemaKey, StagedFile,
    StorageBatch, StorageCursor, StorageSchema, StoreHistory, StoredRecord, StoredRow,
};
use crate::{Error, Result};
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

/// A pinned historical image of the primary store at one saved cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendSnapshot {
    pub at: StorageCursor,
    pub token: Vec<u8>,
}

/// One bounded scan page: entries after `after`, within the row and byte bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanPage {
    pub after: u64,
    pub rows: NonZeroUsize,
    pub bytes: NonZeroUsize,
}

/// Row and byte bounds of one recovery page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadLimits {
    pub rows: NonZeroUsize,
    pub bytes: NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/// The physical store behind a `StorageManager`.
///
/// Scans return entries strictly ascending by storage position within
/// `(page.after, snapshot.at]` that match the selection. A page holds at least
/// one entry while any remain, otherwise it stays within `page.rows` and
/// `page.bytes` of encoded entries; an empty page means the scan is exhausted.
/// Latest object selection returns each selected key's newest revision at the
/// snapshot, ordered by that revision's position.
pub trait StorageBackend: Send + Sync + 'static {
    // Lifecycle
    fn history(&self) -> &StoreHistory;
    fn open(&self) -> StorageFuture<'_, StorageCursor>;
    fn cursor_saved(&self) -> StorageFuture<'_, StorageCursor>;
    fn close(&self) -> StorageFuture<'_, ()>;

    // Atomic Durable Writes
    fn write<'a>(&'a self, batch: StorageBatch<'a>) -> StorageFuture<'a, StorageCursor>;

    // Historical Reads
    fn open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot>;
    fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()>;
    fn read_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>>;
    fn read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>>;
    fn read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>>;
    fn read_stream<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>>;
    fn read_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>>;
    fn read_object<'a>(&'a self, snapshot: &'a BackendSnapshot, point: &'a ObjectPoint) -> StorageFuture<'a, Option<ObjectEntry>>;
    fn read_file<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, max_bytes: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>>;

    // Object Lifecycle Retention
    /// Persist a monotonic release frontier before its stale revisions may be
    /// reclaimed. Repeating a covered release must not advance metadata.
    fn release_object_history<'a>(
        &'a self, schema: &'a SchemaKey, item: &'a ObjectKey, through: &'a StorageCursor,
    ) -> StorageFuture<'a, ()>;

    /// Reclaim released revisions through `through`; `true` when nothing remains.
    fn prune_objects<'a>(&'a self, through: &'a StorageCursor, max_entries: NonZeroUsize) -> StorageFuture<'a, bool>;

    // Finished File Publication
    fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile>;
    fn write_file<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: &'a [u8]) -> StorageFuture<'a, ()>;
    fn finish_file<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference>;
    fn abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()>;

    // Resumable File Sessions
    fn supports_file_sessions(&self) -> bool {
        false
    }
    fn create_production<'a>(&'a self, _metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        unsupported("file session")
    }
    fn fence_production<'a>(&'a self, _file: &'a StagedFile, _expected: u64, _next: u64) -> StorageFuture<'a, ()> {
        unsupported("file session")
    }
    fn save_production<'a>(&'a self, _checkpoint: &'a FileCheckpoint) -> StorageFuture<'a, ()> {
        unsupported("file checkpoint")
    }
    fn recover_productions(&self, _limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        unsupported("file recovery")
    }
    fn file_progress<'a>(&'a self, _file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        unsupported("file following")
    }
    fn read_production<'a>(&'a self, _file: &'a StagedFile, _offset: u64, _max_bytes: NonZeroUsize) -> StorageFuture<'a, Vec<u8>> {
        unsupported("file following")
    }
    fn abandon_file<'a>(&'a self, _staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        unsupported("file session")
    }
    fn supports_file_deletion(&self) -> bool {
        false
    }
    fn delete_file<'a>(&'a self, _file: &'a StagedFile) -> StorageFuture<'a, ()> {
        unsupported("file deletion")
    }

    // Idempotent Mutation Receipts
    fn supports_mutation_receipts(&self) -> bool {
        false
    }
    fn save_mutations<'a>(&'a self, _receipts: &'a [MutationReceipt]) -> StorageFuture<'a, ()> {
        unsupported("storage mutations")
    }

    /// The unexpired receipts with the latest expiry that fit `limits`; older
    /// receipts beyond the bound are omitted, as the in-memory ledger evicts them.
    fn recover_mutations(&self, _limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        unsupported("storage mutations")
    }

    // Stored Identity Catalog
    /// Distinct stream or object identities durable at the snapshot, ordered by
    /// the position where each first appeared, within the scan page contract.
    /// Identities persist across restart in the backend's own metadata index;
    /// a page costs O(page), never a scan of the rows or revisions behind it.
    /// Backends without that index refuse rather than derive identities.
    fn read_identities<'a>(
        &'a self, _snapshot: &'a BackendSnapshot, _selection: &'a IdentitySelection, _page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredIdentity>> {
        unsupported("storage identity catalog")
    }
}

/* === Implementations === */

impl ReadLimits {
    pub fn check(&self, rows: usize, bytes: usize) -> Result<()> {
        if rows > self.rows.get() || bytes > self.bytes.get() {
            return Err(Error::InvalidInput { field: "read page", reason: "row or byte bound exceeded" });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn unsupported<'a, T>(field: &'static str) -> StorageFuture<'a, T> {
    Box::pin(async move { Err(Error::InvalidInput { field, reason: "backend does not support this capability" }) })
}

/* === Tests === */
