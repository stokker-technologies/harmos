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
        Self {
            settings,
            history,
            owner: Arc::new(Mutex::new(None)),
            files: Arc::new(files),
        }
    }

    pub fn into_backends(self, settings: state::StorageStateSettings) -> StorageBackends {
        let state = state::file::FileBackend::new(self.settings.root.join("state"), self.history.clone(), settings);
        StorageBackends {
            primary: Arc::new(self),
            state: Arc::new(state),
        }
    }

    async fn call<T: Send + 'static>(&self, action: impl FnOnce(&mut Log) -> Result<T> + Send + 'static) -> Result<T> {
        if self.files.closing.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "JSON storage" });
        }
        let mut owner = tracing::Instrument::instrument(self.owner.clone().lock_owned(), tracing::trace_span!("storage.lock.wait")).await;
        if self.files.closing.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "JSON storage" });
        }
        {
            let context = crate::execution::Context::current();
            tokio::task::spawn_blocking(move || {
                context.blocking(|| {
                    let log = owner.as_mut().ok_or(Error::NotReady { domain: "JSON storage" })?;
                    action(log)
                })
            })
        }
        .await
        .map_err(task)?
    }

    async fn read<T: Send + 'static>(&self, action: impl FnOnce(&mut Log) -> Result<T> + Send + 'static) -> Result<T> {
        self.call(action).await
    }

    async fn read_selected<T: Send + 'static, S: Clone + MemorySize + Send + Sync + 'static>(
        &self, snapshot: &BackendSnapshot, selection: &S, action: impl FnOnce(&mut Log, &BackendSnapshot, &S) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let snapshot = snapshot.clone();
        let selection = selection.clone();
        self.read(move |log| action(log, &snapshot, &selection)).await
    }

    async fn file_read<T: Send + 'static>(
        &self, file: StagedFile, action: impl FnOnce(&Files, &str) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        self.file(file, action).await
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
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "open", outcome = "cancelled"));
            context
                .result(span, {
                    async move {
                        let mut owner =
                            tracing::Instrument::instrument(self.owner.clone().lock_owned(), tracing::trace_span!("storage.lock.wait"))
                                .await;
                        let settings = self.settings.clone();
                        let history = self.history.clone();
                        let files = self.files.clone();
                        let result = {
                            let context = crate::execution::Context::current();
                            tokio::task::spawn_blocking(move || {
                                context.blocking(|| {
                                    if owner.is_none() {
                                        *owner = Some(Log::open(&settings, history)?);
                                    }
                                    let log = owner.as_ref().expect("opened JSON log");
                                    *files.lease.lock().map_err(|_| format::corrupt("file lease is poisoned"))? =
                                        Arc::downgrade(&log.lease);
                                    files.closing.store(false, Ordering::Release);
                                    Ok(log.cursor())
                                })
                            })
                        }
                        .await
                        .map_err(task)??;
                        Ok(result)
                    }
                })
                .await
        })
    }
    fn cursor_saved(&self) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "cursor_saved", outcome = "cancelled"));
            context.result(span, self.read(|log| Ok(log.cursor()))).await
        })
    }
    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "close", outcome = "cancelled"));
            context
                .result(span, {
                    async move {
                        self.files.closing.store(true, Ordering::Release);
                        let activity = tracing::Instrument::instrument(
                            self.files.activity.clone().write_owned(),
                            tracing::trace_span!("storage.lock.wait"),
                        )
                        .await;
                        let mut owner =
                            tracing::Instrument::instrument(self.owner.clone().lock_owned(), tracing::trace_span!("storage.lock.wait"))
                                .await;
                        {
                            let context = crate::execution::Context::current();
                            tokio::task::spawn_blocking(move || {
                                context.blocking(|| {
                                    let _activity = activity;
                                    owner.take();
                                    Ok(())
                                })
                            })
                        }
                        .await
                        .map_err(task)?
                    }
                })
                .await
        })
    }
    fn write_schemas(&self, batch: StorageBatch<StorageSchema>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "write_schemas", outcome = "cancelled"));
            context.result(span, self.write(batch)).await
        })
    }
    fn write_objects(&self, batch: StorageBatch<ObjectEntry>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "write_objects", outcome = "cancelled"));
            context.result(span, self.write(batch)).await
        })
    }
    fn write_records(&self, batch: StorageBatch<StoredRecord>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "write_records", outcome = "cancelled"));
            context.result(span, self.write(batch)).await
        })
    }
    fn write_stream(&self, batch: StorageBatch<StoredRow>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "write_stream", outcome = "cancelled"));
            context.result(span, self.write(batch)).await
        })
    }
    fn write_files(&self, batch: StorageBatch<FileReference>) -> StorageFuture<'_, StorageCursor> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "write_files", outcome = "cancelled"));
            context
                .result(span, {
                    let files = self.files.clone();
                    self.call(move |log| {
                        for reference in batch.as_slice() {
                            let production = format::load::<files::Production>(
                                &files.metadata_path(&files::key(&reference.metadata.id)),
                                files.maximum,
                            )?
                            .ok_or_else(|| format::invalid("unknown staged file"))?;
                            if production.phase != FilePhase::Finished || production.finish.as_ref() != Some(reference) {
                                return Err(format::conflict("file publication precedes durable finish"));
                            }
                        }
                        log.write(batch)
                    })
                })
                .await
        })
    }
    fn open_snapshot<'a>(&'a self, at: &'a StorageCursor) -> StorageFuture<'a, BackendSnapshot> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "open_snapshot", outcome = "cancelled"));
            context
                .result(span, {
                    async move {
                        let at = at.clone();
                        self.read(move |log| log.snapshot(at)).await
                    }
                })
                .await
        })
    }
    fn release_snapshot<'a>(&'a self, snapshot: &'a BackendSnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "release_snapshot", outcome = "cancelled"));
            context
                .result(span, {
                    let snapshot = snapshot.clone();
                    self.read(move |log| {
                        log.pin(&snapshot)?;
                        log.snapshots.remove(&snapshot.token);
                        log.readers.remove(&snapshot.token);
                        Ok(())
                    })
                })
                .await
        })
    }
    fn read_schemas<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a SchemaSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StorageSchema>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_schemas", outcome = "cancelled"));
            context
                .result(span, {
                    self.read_selected(snapshot, selection, move |log, snapshot, selection| {
                        let generation = log.pin(snapshot)?;
                        log.scan(snapshot.at.sequence, generation, page, |row: &StorageSchema| {
                            Ok(selection.schema.as_ref().is_none_or(|key| key == &row.key))
                        })
                    })
                })
                .await
        })
    }
    fn read_records<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a RecordSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRecord>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_records", outcome = "cancelled"));
            context
                .result(span, {
                    self.read_selected(snapshot, selection, move |log, snapshot, selection| {
                        let generation = log.pin(snapshot)?;
                        log.scan(snapshot.at.sequence, generation, page, |row: &StoredRecord| {
                            Ok(crate::selection::Entry::matches(row, selection))
                        })
                    })
                })
                .await
        })
    }
    fn read_stream<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a StreamSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredRow>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_stream", outcome = "cancelled"));
            context
                .result(span, {
                    self.read_selected(snapshot, selection, move |log, snapshot, selection| {
                        let generation = log.pin(snapshot)?;
                        log.scan(snapshot.at.sequence, generation, page, |row: &StoredRow| {
                            Ok(crate::selection::Entry::matches(row, selection))
                        })
                    })
                })
                .await
        })
    }
    fn read_files<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a FileSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<FileReference>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_files", outcome = "cancelled"));
            context
                .result(span, {
                    self.read_selected(snapshot, selection, move |log, snapshot, selection| {
                        let generation = log.pin(snapshot)?;
                        log.scan(snapshot.at.sequence, generation, page, |row: &FileReference| {
                            Ok(crate::selection::Entry::matches(row, selection))
                        })
                    })
                })
                .await
        })
    }
    fn read_objects<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a ObjectSelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<ObjectEntry>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_objects", outcome = "cancelled"));
            context
                .result(span, {
                    self.read_selected(snapshot, selection, move |log, snapshot, selection| log.objects(snapshot, selection, page))
                })
                .await
        })
    }
    fn read_object<'a>(&'a self, snapshot: &'a BackendSnapshot, point: &'a ObjectPoint) -> StorageFuture<'a, Option<ObjectEntry>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_object", outcome = "cancelled"));
            context.result(span, self.read_selected(snapshot, point, move |log, snapshot, point| log.object(snapshot, point))).await
        })
    }
    fn read_file<'a>(
        &'a self, snapshot: &'a BackendSnapshot, file: &'a FileReference, offset: u64, maximum: NonZeroUsize,
    ) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_file", outcome = "cancelled"));
            context
                .result(span, {
                    async move {
                        let files = Arc::clone(&self.files);
                        self.read_selected(snapshot, file, move |log, snapshot, reference| {
                            log.pin(snapshot)?;
                            if log.readers.get(&snapshot.token).is_none_or(|reader| reader.reference != *reference) {
                                if reference.position.sequence <= snapshot.at.sequence
                                    && log.one::<FileReference>(reference.position.sequence)?.as_ref() != Some(reference)
                                {
                                    return Err(format::conflict("file differs from pinned saved view"));
                                }
                                let key = files::key(&reference.metadata.id);
                                let reader = files.open_reader(&key, reference.clone())?;
                                log.readers.insert(snapshot.token.clone(), reader);
                            }
                            log.readers.get_mut(&snapshot.token).expect("pinned file reader").read(offset, maximum)
                        })
                        .await
                    }
                })
                .await
        })
    }

    fn release_object_history<'a>(
        &'a self, schema: &'a SchemaKey, item: &'a ObjectKey, through: &'a StorageCursor,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span =
                context.sync(|| tracing::debug_span!("storage.json.backend", method = "release_object_history", outcome = "cancelled"));
            context
                .result(span, {
                    let schema = schema.clone();
                    let item = item.clone();
                    let through = through.clone();
                    self.read(move |log| log.release(schema, item, through))
                })
                .await
        })
    }
    fn prune_objects<'a>(
        &'a self, through: &'a StorageCursor, maximum: NonZeroUsize, retention: fn(&SchemaKey) -> harmos_domain::Result<ObjectRetention>,
    ) -> StorageFuture<'a, bool> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "prune_objects", outcome = "cancelled"));
            context
                .result(span, {
                    let through = through.clone();
                    self.read(move |log| log.prune(&through, maximum, retention))
                })
                .await
        })
    }
    fn create_file<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "create_file", outcome = "cancelled"));
            context
                .result(span, {
                    let metadata = metadata.clone();
                    let files = self.files.clone();
                    let id = metadata.id.clone();
                    async move {
                        files
                            .call(&id, move |files, key| {
                                let result = files.create(key, metadata, false);
                                Ok(result)
                            })
                            .await?
                    }
                })
                .await
        })
    }
    fn write_file<'a>(&'a self, staging: &'a StagedFile, offset: u64, bytes: FileChunk) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "write_file", outcome = "cancelled"));

            context.result(span, self.file_read(staging.clone(), move |files, key| files.write(key, offset, bytes.as_slice()))).await
        })
    }
    fn finish_file<'a>(
        &'a self, staging: &'a StagedFile, position: &'a StorageCursor, length: u64, sha256: [u8; 32],
    ) -> StorageFuture<'a, FileReference> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "finish_file", outcome = "cancelled"));
            context
                .result(span, {
                    let position = position.clone();
                    self.file_read(staging.clone(), move |files, key| files.finish(key, position, length, sha256))
                })
                .await
        })
    }
    fn abort_file<'a>(&'a self, staging: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "abort_file", outcome = "cancelled"));

            context.result(span, self.file_read(staging.clone(), move |files, key| files.abort(key, false, false))).await
        })
    }
    fn supports_file_sessions(&self) -> bool {
        true
    }
    fn create_production<'a>(&'a self, metadata: &'a FileMetadata) -> StorageFuture<'a, StagedFile> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "create_production", outcome = "cancelled"));
            context
                .result(span, {
                    let metadata = metadata.clone();
                    let files = self.files.clone();
                    let id = metadata.id.clone();
                    async move {
                        files
                            .call(&id, move |files, key| {
                                let result = files.create(key, metadata, true);
                                Ok(result)
                            })
                            .await?
                    }
                })
                .await
        })
    }
    fn fence_production<'a>(&'a self, file: &'a StagedFile, expected: u64, next: u64) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "fence_production", outcome = "cancelled"));

            context.result(span, self.file_read(file.clone(), move |files, key| files.fence(key, expected, next))).await
        })
    }
    fn save_production(&self, checkpoint: StorageBatch<FileCheckpoint>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "save_production", outcome = "cancelled"));
            context
                .result(span, {
                    async move {
                        let [value] = checkpoint.as_slice() else {
                            return Err(format::invalid("checkpoint requires one value"));
                        };
                        let file = StagedFile {
                            history: value.session.history.clone(),
                            id: value.session.id.clone(),
                        };
                        self.file(file, move |files, key| files.checkpoint(key, &checkpoint.as_slice()[0])).await
                    }
                })
                .await
        })
    }
    fn recover_productions(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<FileCheckpoint>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "recover_productions", outcome = "cancelled"));
            context
                .result(span, {
                    let files = self.files.clone();
                    self.read(move |_| files.recover(limits))
                })
                .await
        })
    }
    fn file_progress<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, FileProgress> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "file_progress", outcome = "cancelled"));
            context.result(span, async move { self.file_read(file.clone(), move |files, key| files.progress(key)).await }).await
        })
    }

    fn read_production<'a>(&'a self, file: &'a StagedFile, offset: u64, maximum: NonZeroUsize) -> StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_production", outcome = "cancelled"));
            context
                .result(span, {
                    async move { self.file_read(file.clone(), move |files, key| files.read(key, offset, maximum, None)).await }
                })
                .await
        })
    }

    fn abandon_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "abandon_file", outcome = "cancelled"));

            context.result(span, self.file_read(file.clone(), move |files, key| files.abort(key, true, false))).await
        })
    }
    fn supports_file_deletion(&self) -> bool {
        true
    }
    fn delete_file<'a>(&'a self, file: &'a StagedFile) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "delete_file", outcome = "cancelled"));

            context.result(span, self.file_read(file.clone(), move |files, key| files.abort(key, false, true))).await
        })
    }
    fn supports_mutation_receipts(&self) -> bool {
        true
    }
    fn save_mutations(&self, receipts: StorageBatch<MutationReceipt>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "save_mutations", outcome = "cancelled"));
            context
                .result(span, {
                    let files = self.files.clone();
                    self.call(move |log| {
                        for receipt in receipts.as_slice() {
                            files.save_receipt(receipt, log.saved)?;
                        }
                        Ok(())
                    })
                })
                .await
        })
    }
    fn recover_mutations(&self, limits: ReadLimits) -> StorageFuture<'_, Vec<MutationReceipt>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "recover_mutations", outcome = "cancelled"));
            context
                .result(span, {
                    let files = self.files.clone();
                    self.read(move |_| files.receipts(limits))
                })
                .await
        })
    }
    fn read_identities<'a>(
        &'a self, snapshot: &'a BackendSnapshot, selection: &'a IdentitySelection, page: ScanPage,
    ) -> StorageFuture<'a, Vec<StoredIdentity>> {
        Box::pin(async move {
            let context = crate::execution::Context::current();
            let span = context.sync(|| tracing::debug_span!("storage.json.backend", method = "read_identities", outcome = "cancelled"));
            context
                .result(span, {
                    self.read_selected(snapshot, selection, move |log, snapshot, selection| log.catalog(snapshot, selection, page))
                })
                .await
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn task(error: tokio::task::JoinError) -> Error {
    Error::backend("JSON storage task", BackendFailure::Permanent, error)
}

/* === Tests === */
