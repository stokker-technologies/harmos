/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Own the Query Benchmark Storage Worker

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Local Imports
use harmos_storage::memory::{Backend, RecoveryBackend};
use harmos_storage::{
    Error, Identifier, Result, StorageBackends, StorageFilesystemSettings, StorageFuture, StorageManager, StorageRequestSettings,
    StorageResponseSettings, StorageSettings, StoreHistory, WorkerOwner,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Fixture {
    pub(crate) storage: StorageManager,
}

struct Task(Mutex<Option<JoinHandle<Result<()>>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl WorkerOwner for Task {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            if let Some(task) = self.0.lock().await.take() {
                task.await.map_err(|source| Error::OperationFailed {
                    operation: "join query benchmark worker".into(),
                    source: Arc::new(source),
                })??;
            }
            Ok(())
        })
    }
}

impl Fixture {
    pub(crate) async fn new(settings: StorageSettings) -> Self {
        let backends = StorageBackends {
            primary: Arc::new(Backend::new(history())),
            state: Arc::new(RecoveryBackend::new(history())),
        };
        let storage = StorageManager::new(backends, settings).unwrap();
        storage
            .start_with(Arc::new(()), |mut worker| {
                let task = tokio::spawn(async move {
                    worker.run().await?;
                    worker.cleanup().await
                });
                Ok(Box::new(Task(Mutex::new(Some(task)))))
            })
            .await
            .unwrap();
        Self { storage }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
    }
}

/* === Public Functions === */

pub(crate) fn history() -> StoreHistory {
    StoreHistory {
        store: Identifier::new("benchmark-store").unwrap(),
        epoch: Identifier::new("benchmark-epoch").unwrap(),
    }
}

pub(crate) fn config() -> StorageSettings {
    StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        filesystem: StorageFilesystemSettings {
            poll_interval: Duration::from_millis(100),
            max_entries: NonZeroUsize::new(100_000).unwrap(),
            event_capacity: NonZeroUsize::new(1024).unwrap(),
        },

        request: StorageRequestSettings {
            max_file_productions: NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: NonZeroUsize::new(1024).unwrap(),
            max_pending_queries: nz(64),
            max_active_queries: nz(4),
            max_pending_write_batches: nz(256),
            max_active_file_operations: nz(1),
            max_pending_write_bytes: nz(16 * 1024 * 1024),
        },
        response: StorageResponseSettings {
            batch_rows: nz(64),
            batch_bytes: nz(64 * 1024),
        },
    }
}

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
