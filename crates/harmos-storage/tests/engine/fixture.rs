/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Own The Storage Engine Worker In Tests

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
pub(crate) use std::num::{NonZeroU32, NonZeroUsize};
pub(crate) use std::sync::Arc;
pub(crate) use std::sync::atomic::Ordering;
pub(crate) use std::time::Duration;

// Third Party Imports
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Local Imports
pub(crate) use super::backend::{Data, Recovery, history, storage_settings};
pub(crate) use harmos_storage::*;


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Fixture {
    pub(crate) storage: StorageManager,
    pub(crate) data: Arc<Data>,
    pub(crate) recovery: Arc<Recovery>,
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
                    operation: "join storage test worker".into(),
                    source: Arc::new(source),
                })??;
            }
            Ok(())
        })
    }
}

impl Fixture {
    pub(crate) async fn new(config: StorageSettings) -> Self {
        let data = Arc::new(Data::new(history()));
        let recovery = Arc::new(Recovery::new(history()));
        let storage = StorageManager::new(StorageBackends {
            primary: data.clone(), state: recovery.clone(),
        }, config).unwrap();
        start(&storage).await;
        Self { storage, data, recovery }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
        assert!(self.storage.status().closed);
        assert!(!self.storage.status().running);
    }
}

/* === Public Functions === */
pub(crate) async fn start<C>(storage: &StorageManager<C>) {
    storage.start_with(Arc::new(()), |mut worker| {
            let task = tokio::spawn(async move {
                worker.run().await?;
                worker.cleanup().await
            });
            Ok(Box::new(Task(Mutex::new(Some(task)))))
        }).await.unwrap();
}

pub(crate) fn state_settings() -> harmos_storage::state::StorageStateSettings {
    harmos_storage::state::StorageStateSettings {
        max_entry_bytes: std::num::NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
        max_checkpoint_bytes: std::num::NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
    }
}

/* === Private Functions === */
pub(crate) fn config() -> StorageSettings {
    storage_settings()
}

pub(crate) fn schema() -> SchemaKey {
    SchemaKey {
        payload_encoding: harmos_storage::PayloadEncoding::PostcardV1,
        id: Identifier::new("reading").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
    }
}

pub(crate) fn record<'a>(
    schema: &'a SchemaKey,
    item: &'a Identifier,
    payload: &'a [u8],
) -> RecordWrite<'a> {
    RecordWrite {
        schema,
        scope: None,
        item,
        source: None,
        source_time: None,
        payload,
    }
}

/// Whether one poll of `future` leaves it pending, without waiting for anything.
pub(crate) async fn pending<F: std::future::Future + ?Sized>(
    mut future: std::pin::Pin<&mut F>,
) -> bool {
    std::future::poll_fn(|context| {
        std::task::Poll::Ready(future.as_mut().poll(context).is_pending())
    })
    .await
}

pub(crate) async fn write<C>(storage: &StorageManager<C>, id: &str) -> StorageCursor {
    storage
        .write_records(&[record(&schema(), &Identifier::new(id).unwrap(), &[1, 2, 3])])
        .await
        .unwrap()
}



/* === Tests === */
