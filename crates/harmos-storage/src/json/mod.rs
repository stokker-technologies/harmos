/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded JSON Collection Storage And Native File Publication.

/* === Modules === */

mod config;
mod files;
mod format;
mod log;
mod reads;
mod sessions;

/* === Re-Exports === */

pub use config::JsonStorageSettings;

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
};

// Third Party Imports
use tokio::sync::Mutex;

// Local Imports
use crate::arrow::RetainedMemory;
use crate::{
    query::{FileSelection, ObjectSelection, RecordSelection, SchemaSelection, StreamSelection},
    *,
};
use files::Files;
use log::{Log, Row};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct JsonStorage {
    settings: JsonStorageSettings,
    history: StoreHistory,
    owner: Arc<Mutex<Option<Log>>>,
    files: Arc<Files>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl JsonStorage {
    pub fn new(settings: JsonStorageSettings, history: StoreHistory) -> Self {
        let files = Files::new(settings.root.clone(), history.clone(), settings.max_entry_bytes.get());
        Self { settings, history, owner: Arc::new(Mutex::new(None)), files: Arc::new(files) }
    }

    pub fn into_backends(self, settings: state::StorageStateSettings) -> StorageBackends {
        let state = state::file::FileBackend::new(self.settings.root.join("state"), self.history.clone(), settings);
        StorageBackends { primary: Arc::new(self), state: Arc::new(state) }
    }

    async fn call<T: Send + 'static>(&self, action: impl FnOnce(&mut Log) -> Result<T> + Send + 'static) -> Result<T> {
        if self.files.closing.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "JSON storage" });
        }
        let mut owner = self.owner.clone().lock_owned().await;
        if self.files.closing.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "JSON storage" });
        }
        tokio::task::spawn_blocking(move || {
            let log = owner.as_mut().ok_or(Error::NotReady { domain: "JSON storage" })?;
            action(log)
        })
        .await
        .map_err(task)?
    }

    async fn read<T: Send + 'static>(
        &self, memory: RetainedMemory, action: impl FnOnce(&mut Log, &RetainedMemory) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (result, _memory) = self.call(move |log| Ok((action(log, &memory), memory))).await?;
        result
    }

    async fn read_selected<T: Send + 'static, S: Clone + MemorySize + Send + Sync + 'static>(
        &self, snapshot: &BackendSnapshot, selection: &S, memory: RetainedMemory,
        action: impl FnOnce(&mut Log, &BackendSnapshot, &S, &RetainedMemory, usize) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let base = snapshot
            .at
            .memory_size()
            .saturating_add(snapshot.token.capacity())
            .saturating_add(std::mem::size_of::<Vec<u8>>())
            .saturating_add(selection.memory_size())
            .saturating_add(crate::manager::ENTRY_OVERHEAD);
        memory.try_grow_to(base)?;
        let snapshot = snapshot.clone();
        let selection = selection.clone();
        self.read(memory, move |log, memory| action(log, &snapshot, &selection, memory, base)).await
    }

    async fn file_read<T: Send + 'static>(
        &self, file: StagedFile, memory: RetainedMemory, action: impl FnOnce(&Files, &str) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (result, _memory) = self.file(file, move |files, key| Ok((action(files, key), memory))).await?;
        result
    }

    async fn write<T: Row>(&self, batch: StorageBatch<T>) -> Result<StorageCursor> {
        self.call(move |log| log.write(batch)).await
    }

    async fn file<T: Send + 'static>(
        &self, file: StagedFile, action: impl FnOnce(&Files, &str) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.history.check(&file.history)?;
        self.files.call(&file.id, action).await
    }
}

impl StorageBackend for JsonStorage {
    fn history(&self) -> &StoreHistory {
        &self.history
    }
    fn open(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let mut owner = self.owner.clone().lock_owned().await;
            let settings = self.settings.clone();
            let history = self.history.clone();
            let files = self.files.clone();
            tokio::task::spawn_blocking(move || {
                if owner.is_none() {
                    *owner = Some(Log::open(&settings, history)?);
                }
                let log = owner.as_ref().expect("opened JSON log");
                *files.lease.lock().map_err(|_| format::corrupt("file lease is poisoned"))? = Arc::downgrade(&log.lease);
                files.closing.store(false, Ordering::Release);
                Ok(log.cursor())
            })
            .await
            .map_err(task)?
        })
    }
    fn cursor_saved(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.call(|log| Ok(log.cursor())))
    }
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            self.files.closing.store(true, Ordering::Release);
            let activity = self.files.activity.clone().write_owned().await;
            let mut owner = self.owner.clone().lock_owned().await;
            tokio::task::spawn_blocking(move || {
                let _activity = activity;
                owner.take();
                Ok(())
            })
            .await
            .map_err(task)?
        })
    }
    fn write_schemas(&self, batch: StorageBatch<StorageSchema>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.write(batch))
    }
    fn write_objects(&self, batch: StorageBatch<ObjectEntry>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.write(batch))
    }
    fn write_records(&self, batch: StorageBatch<StoredRecord>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.write(batch))
    }
    fn write_stream(&self, batch: StorageBatch<StoredRow>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(self.write(batch))
    }
    fn write_files(&self, batch: StorageBatch<FileReference>) -> StorageFuture<'_, StorageCursor> {
        let files = self.files.clone();
        Box::pin(self.call(move |log| {
            for reference in batch.as_slice() {
                let production = files.load(&files::key(&reference.metadata.id))?;
                if production.phase != FilePhase::Finished || production.finish.as_ref() != Some(reference) {
                    return Err(format::conflict("file publication precedes durable finish"));
                }
            }
            log.write(batch)
        }))
    }
    fn open_snapshot<'a>(&'a self, at: &'a StorageCursor, memory: RetainedMemory) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            let base =
                at.memory_size().saturating_add(std::mem::size_of::<BackendSnapshot>()).saturating_add(32 + crate::manager::ENTRY_OVERHEAD);
            memory.try_grow_to(base)?;
            let at = at.clone();
            self.read(memory, move |log, _memory| log.snapshot(at)).await
        })
    }
    fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        let snapshot = snapshot.clone();
        Box::pin(self.call(move |log| {
            log.pin(&snapshot)?;
            log.snapshots.remove(&snapshot.token);
            Ok(())
        }))
    }
    fn read_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        Box::pin(self.read_selected(snapshot, selection, memory, move |log, snapshot, selection, memory, base| {
            let generation = log.pin(snapshot)?;
            log.scan(snapshot.at.sequence, generation, page, memory, base, |row: &StorageSchema| {
                Ok(selection.schema.as_ref().is_none_or(|key| key == &row.key))
            })
        }))
    }
    fn read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        Box::pin(self.read_selected(snapshot, selection, memory, move |log, snapshot, selection, memory, base| {
            let generation = log.pin(snapshot)?;
            log.scan(snapshot.at.sequence, generation, page, memory, base, |row: &StoredRecord| {
                Ok(crate::selection::Entry::matches(row, selection))
            })
        }))
    }
    fn read_stream<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        Box::pin(self.read_selected(snapshot, selection, memory, move |log, snapshot, selection, memory, base| {
            let generation = log.pin(snapshot)?;
            log.scan(snapshot.at.sequence, generation, page, memory, base, |row: &StoredRow| {
                Ok(crate::selection::Entry::matches(row, selection))
            })
        }))
    }
    fn read_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        Box::pin(self.read_selected(snapshot, selection, memory, move |log, snapshot, selection, memory, base| {
            let generation = log.pin(snapshot)?;
            log.scan(snapshot.at.sequence, generation, page, memory, base, |row: &FileReference| {
                Ok(crate::selection::Entry::matches(row, selection))
            })
        }))
    }
    fn read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        Box::pin(self.read_selected(snapshot, selection, memory, move |log, snapshot, selection, memory, base| {
            log.objects(snapshot, selection, page, memory, base)
        }))
    }
    fn read_object<'a>(
        &'a self, snapshot: &'a BackendSnapshot, point: &'a ObjectPoint, memory: RetainedMemory,
    ) -> StorageFuture<'a, Option<ObjectEntry>> {
        Box::pin(
            self.read_selected(snapshot, point, memory, move |log, snapshot, point, memory, base| {
                log.object(snapshot, point, memory, base)
            }),
        )
    }
    fn read_file<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, maximum: NonZeroUsize, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            self.read_selected(snapshot, file, memory.clone(), |log, snapshot, reference, memory, base| {
                log.pin(snapshot)?;
                if reference.position.sequence <= snapshot.at.sequence
                    && log.one::<FileReference>(reference.position.sequence, memory, base)?.as_ref() != Some(reference)
                {
                    return Err(format::conflict("file differs from pinned saved view"));
                }
                Ok(())
            })
            .await?;
            let base = file.memory_size().saturating_mul(2).saturating_add(crate::manager::ENTRY_OVERHEAD);
            memory.try_grow_to(base)?;
            let reference = file.clone();
            let file = StagedFile { history: reference.position.history.clone(), id: reference.metadata.id.clone() };
            let scratch = memory.clone();
            self.file_read(file, memory, move |files, key| files.read(key, offset, maximum, Some(&reference), &scratch, base)).await
        })
    }

    fn release_object_history<'a>(
        &'a self, schema: &'a SchemaKey, item: &'a ObjectKey, through: &'a StorageCursor,
    ) -> StorageFuture<'a, ()> {
        let schema = schema.clone();
        let item = item.clone();
        let through = through.clone();
        Box::pin(self.call(move |log| log.release(schema, item, through)))
    }
    fn prune_objects<'a>(
        &'a self, through: &'a StorageCursor, maximum: NonZeroUsize, retention: fn(&SchemaKey) -> harmos_domain::Result<ObjectRetention>,
    ) -> StorageFuture<'a, bool> {
        let through = through.clone();
        Box::pin(self.call(move |log| log.prune(&through, maximum, retention)))
    }
    fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        let metadata = metadata.clone();
        let files = self.files.clone();
        let id = metadata.id.clone();
        Box::pin(async move { files.call(&id, move |files, key| files.create(key, metadata, false)).await })
    }
    fn write_file<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: FileChunk) -> StorageFuture<'a, ()> {
        Box::pin(self.file(staging.clone(), move |files, key| files.write(key, offset, bytes.as_slice())))
    }
    fn finish_file<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        let position = position.clone();
        Box::pin(self.file(staging.clone(), move |files, key| files.finish(key, position, length, sha256)))
    }
    fn abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(self.file(staging.clone(), |files, key| files.abort(key, false, false)))
    }
    fn supports_file_sessions(&self) -> bool {
        true
    }
    fn create_production<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        let metadata = metadata.clone();
        let files = self.files.clone();
        let id = metadata.id.clone();
        Box::pin(async move { files.call(&id, move |files, key| files.create(key, metadata, true)).await })
    }
    fn fence_production<'a>(&'a self, file: &'a StagedFile, expected: u64, next: u64) -> StorageFuture<'a, ()> {
        Box::pin(self.file(file.clone(), move |files, key| files.fence(key, expected, next)))
    }
    fn save_production(&self, checkpoint: StorageBatch<FileCheckpoint>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let [value] = checkpoint.as_slice() else {
                return Err(format::invalid("checkpoint requires one value"));
            };
            let file = StagedFile { history: value.session.history.clone(), id: value.session.id.clone() };
            self.file(file, move |files, key| files.checkpoint(key, &checkpoint.as_slice()[0])).await
        })
    }
    fn recover_productions(&self, limits: ReadLimits, memory: RetainedMemory) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        let files = self.files.clone();
        Box::pin(self.read(memory, move |_, memory| files.recover(limits, memory)))
    }
    fn file_progress<'a>(&'a self, file: &'a StagedFile, memory: RetainedMemory) -> StorageFuture<'a, FileProgress> {
        Box::pin(async move {
            let base = file.memory_size().saturating_add(crate::manager::ENTRY_OVERHEAD);
            memory.try_grow_to(base)?;
            let scratch = memory.clone();
            self.file_read(file.clone(), memory, move |files, key| files.progress(key, &scratch, base)).await
        })
    }

    fn read_production<'a>(
        &'a self, file: &'a StagedFile, offset: u64, maximum: NonZeroUsize, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let base = file.memory_size().saturating_add(crate::manager::ENTRY_OVERHEAD);
            memory.try_grow_to(base)?;
            let scratch = memory.clone();
            self.file_read(file.clone(), memory, move |files, key| files.read(key, offset, maximum, None, &scratch, base)).await
        })
    }

    fn abandon_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(self.file(file.clone(), |files, key| files.abort(key, true, false)))
    }
    fn supports_file_deletion(&self) -> bool {
        true
    }
    fn delete_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(self.file(file.clone(), |files, key| files.abort(key, false, true)))
    }
    fn supports_mutation_receipts(&self) -> bool {
        true
    }
    fn save_mutations(&self, receipts: StorageBatch<MutationReceipt>) -> StorageFuture<'_, ()> {
        let files = self.files.clone();
        Box::pin(self.call(move |log| {
            for receipt in receipts.as_slice() {
                files.save_receipt(receipt, log.saved)?;
            }
            Ok(())
        }))
    }
    fn recover_mutations(&self, limits: ReadLimits, memory: RetainedMemory) -> StorageFuture<'_, Vec<MutationReceipt>> {
        let files = self.files.clone();
        Box::pin(self.read(memory, move |_, memory| files.receipts(limits, memory)))
    }
    fn read_identities<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a IdentitySelection, page: ScanPage, memory: RetainedMemory,
    ) -> StorageFuture<'a, Vec<StoredIdentity>> {
        Box::pin(self.read_selected(snapshot, selection, memory, move |log, snapshot, selection, memory, base| {
            log.catalog(snapshot, selection, page, memory, base)
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn task(error: tokio::task::JoinError) -> Error {
    Error::backend("JSON storage task", BackendFailure::Permanent, error)
}

/* === Tests === */
