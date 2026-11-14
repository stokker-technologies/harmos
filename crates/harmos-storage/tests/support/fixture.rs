/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Own the Storage Engine Worker in Tests

/* === Imports === */

// Standard Library Imports
pub(crate) use std::num::NonZeroU32;
pub(crate) use std::sync::Arc;

// Third Party Imports
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Local Imports
pub(crate) use super::backend::Backend;
pub(crate) use super::backend::RecoveryBackend;
pub(crate) use super::backend::history;
pub(crate) use super::backend::storage_settings;
pub(crate) use harmos_storage::*;

/* === Structs === */

pub(crate) struct Fixture<M = StorageManager> {
    pub(crate) storage: M,
    pub(crate) data: Arc<Backend>,
    pub(crate) recovery: Arc<RecoveryBackend>,
}

struct Task(Mutex<Option<JoinHandle<Result<()>>>>);

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
        let fixture = Self::build_with(config, |backends, config| StorageManager::new(backends, config).unwrap());
        start(&fixture.storage).await;
        fixture
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
    }
}

impl<M> Fixture<M> {
    pub(crate) fn build_with(config: StorageSettings, create: impl FnOnce(StorageBackends, StorageSettings) -> M) -> Self {
        let data = Arc::new(Backend::new(history()));
        let recovery = Arc::new(RecoveryBackend::new(history()));
        let storage = create(
            StorageBackends {
                primary: data.clone(),
                state: recovery.clone(),
            },
            config,
        );
        Self { storage, data, recovery }
    }
}

/* === Public Functions === */

pub(crate) async fn start(storage: &StorageManager) {
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

pub(crate) fn record<'a>(schema: &'a SchemaKey, item: &'a Identifier, payload: &'a [u8]) -> RecordInput {
    RecordInput {
        schema: schema.clone(),
        scope: None,
        item: item.clone(),
        source: None,
        source_time: None,
        payload: (payload).to_vec(),
    }
}

pub(crate) async fn write(storage: &StorageManager, id: &str) -> StorageCursor {
    storage.append_records(vec![record(&schema(), &Identifier::new(id).unwrap(), &[1, 2, 3])]).await.unwrap()
}
