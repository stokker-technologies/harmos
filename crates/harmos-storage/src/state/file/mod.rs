/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Durable JSON State Backend

/* === Modules === */

mod files;
mod format;
mod journal;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    fs::{File, OpenOptions},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};
// Third Party Imports
use tokio::sync::Mutex;
// Local Imports
use super::{RecoveryEntry, RecoveryPage, RecoverySnapshot, StateBackend, StorageStateSettings};
use crate::storage::{ReadLimits, RecoveryPosition, StorageBatch, StorageFuture, StoreHistory};
use crate::{BackendFailure, Error, Result};
use files::JournalOpener;
use journal::StateLog;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct FileBackend {
    root: PathBuf,
    history: StoreHistory,
    settings: StorageStateSettings,
    opener: JournalOpener,
    journal: Arc<Mutex<Option<StateLog>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl FileBackend {
    pub fn new(root: PathBuf, history: StoreHistory, settings: StorageStateSettings) -> Self {
        Self::with_journal_opener(root, history, settings, |path| {
            OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)
        })
    }

    // The Opener Must Return The Named Journal Without Truncating Existing Bytes.
    // It Is Called Under The Exclusive Root Lease, Including After Replacement.
    pub fn with_journal_opener(
        root: PathBuf, history: StoreHistory, settings: StorageStateSettings,
        opener: impl Fn(&Path) -> std::io::Result<File> + Send + Sync + 'static,
    ) -> Self {
        let opener = Arc::new(opener);
        let journal = Mutex::new(None);
        let journal = Arc::new(journal);
        Self { root, history, settings, opener, journal }
    }

    async fn call<T: Send + 'static>(&self, action: impl FnOnce(&mut StateLog) -> Result<T> + Send + 'static) -> Result<T> {
        // Acquire Before Dispatch So Cancellation Cannot Enqueue An Older Write.
        let mut owner = self.journal.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let journal = owner.as_mut().ok_or_else(|| format::invalid("state backend is closed"))?;
            journal.check()?;
            action(journal)
        })
        .await
        .map_err(task)?
    }
    async fn read<T: Send + 'static>(
        &self, memory: crate::arrow::RetainedMemory, base: usize,
        action: impl FnOnce(&mut StateLog, &crate::arrow::RetainedMemory) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        memory.try_grow_to(base.saturating_add(8192))?;
        let mut owner = self.journal.clone().lock_owned().await;
        let (result, _memory) = read_task(memory, move |memory| {
            let journal = owner.as_mut().ok_or_else(|| format::invalid("state backend is closed"))?;
            journal.check()?;
            action(journal, memory)
        })
        .await
        .map_err(task)?;
        result
    }
}

impl StateBackend for FileBackend {
    fn history(&self) -> &StoreHistory {
        &self.history
    }

    fn open(&self) -> StorageFuture<'_, RecoveryPosition> {
        Box::pin(async move {
            let mut owner = self.journal.clone().lock_owned().await;
            let root = self.root.clone();
            let history = self.history.clone();
            let settings = self.settings;
            let opener = Arc::clone(&self.opener);
            tokio::task::spawn_blocking(move || {
                if owner.is_none() {
                    *owner = Some(StateLog::open(&root, history, settings, opener)?);
                }
                let journal = owner.as_ref().expect("opened journal");
                journal.check()?;
                Ok(journal.position())
            })
            .await
            .map_err(task)?
        })
    }

    fn read_snapshot(&self, maximum: NonZeroUsize, memory: crate::arrow::RetainedMemory) -> StorageFuture<'_, Option<RecoverySnapshot>> {
        Box::pin(self.read(memory, crate::manager::ENTRY_OVERHEAD, move |journal, memory| {
            journal.snapshot_retained(maximum.get(), memory, crate::manager::ENTRY_OVERHEAD)
        }))
    }

    fn write_snapshot(&self, snapshot: StorageBatch<RecoverySnapshot>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if snapshot.len() != 1 {
                return Err(format::invalid("snapshot write requires exactly one snapshot"));
            }
            self.history.check(&snapshot.as_slice()[0].through.history)?;
            self.call(move |journal| journal.checkpoint(&snapshot.as_slice()[0])).await
        })
    }

    fn read_entries<'a>(
        &'a self, after: &'a RecoveryPosition, through: &'a RecoveryPosition, limits: ReadLimits, memory: crate::arrow::RetainedMemory,
    ) -> StorageFuture<'a, RecoveryPage> {
        Box::pin(async move {
            self.history.check(&after.history)?;
            self.history.check(&through.history)?;
            let base = crate::manager::ENTRY_OVERHEAD
                .saturating_add(crate::MemorySize::memory_size(&after.history))
                .saturating_add(crate::MemorySize::memory_size(&through.history));
            memory.try_grow_to(base)?;
            let after = after.clone();
            let through = through.clone();
            self.read(memory, base, move |journal, memory| journal.page(&after, &through, limits, memory, base)).await
        })
    }

    fn write_entries(&self, entries: StorageBatch<RecoveryEntry>) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            for entry in entries.as_slice() {
                self.history.check(&entry.position.history)?;
            }
            self.call(move |journal| journal.append(entries.as_slice())).await
        })
    }

    fn flush<'a>(&'a self, through: &'a RecoveryPosition) -> StorageFuture<'a, RecoveryPosition> {
        Box::pin(async move {
            self.history.check(&through.history)?;
            let through = through.clone();
            self.call(move |journal| journal.flush(&through)).await
        })
    }

    fn close(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let mut owner = self.journal.clone().lock_owned().await;
            tokio::task::spawn_blocking(move || {
                let Some(mut journal) = owner.take() else {
                    return Ok(());
                };
                journal.check()?;
                journal.flush(&journal.position())?;
                Ok(())
            })
            .await
            .map_err(task)?
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn read_task<T: Send + 'static>(
    memory: crate::arrow::RetainedMemory, action: impl FnOnce(&crate::arrow::RetainedMemory) -> Result<T> + Send + 'static,
) -> tokio::task::JoinHandle<(Result<T>, crate::arrow::RetainedMemory)> {
    tokio::task::spawn_blocking(move || {
        let result = action(&memory);
        (result, memory)
    })
}

fn task(error: tokio::task::JoinError) -> Error {
    Error::backend("state file task", BackendFailure::Permanent, error)
}

/* === Tests === */
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_backend::{Data, Recovery, history, storage_settings};

    fn budget() -> crate::StorageManager {
        crate::StorageManager::new(
            crate::StorageBackends { primary: Arc::new(Data::<()>::new(history())), state: Arc::new(Recovery::new(history())) },
            storage_settings(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn detached_read_retained() {
        let budget = budget();
        let memory = budget.read_memory(NonZeroUsize::new(8192).unwrap()).await.unwrap();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let task = read_task(memory, move |_| {
            let bytes = vec![7u8; 4096];
            entered.send(()).unwrap();
            gate.recv().unwrap();
            Ok(bytes)
        });
        ready.await.unwrap();
        drop(task);
        assert_eq!(budget.status().read_bytes, 8192);
        let mut changed = budget.inner.changed.subscribe();
        release.send(()).unwrap();
        while budget.status().read_bytes != 0 {
            changed.changed().await.unwrap();
        }
    }

    #[tokio::test]
    async fn completed_read_retained() {
        let budget = budget();
        let memory = budget.read_memory(NonZeroUsize::new(8192).unwrap()).await.unwrap();
        let task = read_task(memory, |_| Ok(vec![7u8; 4096]));
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }
        assert_eq!(budget.status().read_bytes, 8192);
        drop(task);
        assert_eq!(budget.status().read_bytes, 0);
    }
}
