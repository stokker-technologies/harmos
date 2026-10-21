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
use crate::storage::{ReadLimits, RecoveryPosition, StorageFuture, StoreHistory};
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

    fn read_snapshot(&self, maximum: NonZeroUsize) -> StorageFuture<'_, Option<RecoverySnapshot>> {
        Box::pin(self.call(move |journal| journal.snapshot(maximum.get())))
    }

    fn write_snapshot<'a>(&'a self, snapshot: &'a RecoverySnapshot) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.history.check(&snapshot.through.history)?;
            let snapshot = snapshot.clone();
            self.call(move |journal| journal.checkpoint(&snapshot)).await
        })
    }

    fn read_entries<'a>(
        &'a self, after: &'a RecoveryPosition, through: &'a RecoveryPosition, limits: ReadLimits,
    ) -> StorageFuture<'a, RecoveryPage> {
        Box::pin(async move {
            self.history.check(&after.history)?;
            self.history.check(&through.history)?;
            let after = after.clone();
            let through = through.clone();
            self.call(move |journal| journal.page(&after, &through, limits)).await
        })
    }

    fn write_entries<'a>(&'a self, entries: &'a [RecoveryEntry]) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            for entry in entries {
                self.history.check(&entry.position.history)?;
            }
            let entries = entries.to_vec();
            self.call(move |journal| journal.append(&entries)).await
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

fn task(error: tokio::task::JoinError) -> Error {
    Error::backend("state file task", BackendFailure::Permanent, error)
}

/* === Tests === */
