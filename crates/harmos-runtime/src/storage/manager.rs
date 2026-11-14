/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

pub(crate) use harmos_storage::encoded_size;

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports
use harmos_storage::{
    BackendFailure, Error as StorageError, StorageFuture, StorageManager as EngineManager, Worker as EngineWorker, WorkerOwner,
};

// Local Imports
use super::{ObjectCatalog, StorageBackends, StorageFilesystemSettings, StorageRequestSettings, StorageResponseSettings, StorageSettings};
use crate::operations::{Manager, resident::Lease};
use crate::services::{Once, Resident, ServiceContext, ServiceDefinition};
use crate::{Application, Error, Result, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct StorageManager {
    engine: EngineManager,
}

struct Worker(EngineWorker);

struct Owner {
    leases: Mutex<Vec<Lease>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Clone for StorageManager {
    fn clone(&self) -> Self {
        Self { engine: self.engine.clone() }
    }
}

impl Deref for StorageManager {
    type Target = EngineManager;
    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl StorageManager {
    pub(crate) fn default_in_memory() -> Result<Self> {
        Self::in_memory(
            crate::plugin::manager::catalog(),
            StorageSettings {
                transfer: Default::default(),
                policy: Default::default(),
                request: StorageRequestSettings {
                    max_file_productions: NonZeroUsize::new(256).unwrap(),
                    max_mutation_receipts: NonZeroUsize::new(4096).unwrap(),
                    max_pending_queries: NonZeroUsize::new(64).unwrap(),
                    max_active_queries: NonZeroUsize::new(4).unwrap(),
                    max_pending_write_batches: NonZeroUsize::new(256).unwrap(),
                    max_active_file_operations: NonZeroUsize::new(4).unwrap(),
                    max_pending_write_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
                },
                response: StorageResponseSettings {
                    batch_rows: NonZeroUsize::new(1024).unwrap(),
                    batch_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
                },
                filesystem: StorageFilesystemSettings {
                    poll_interval: Duration::from_millis(100),
                    max_entries: NonZeroUsize::new(100_000).unwrap(),
                    event_capacity: NonZeroUsize::new(1024).unwrap(),
                },
            },
        )
    }

    #[tracing::instrument(target = "harmos::storage", name = "storage.new", level = "debug", skip_all)]
    pub fn in_memory(catalog: ObjectCatalog, settings: StorageSettings) -> Result<Self> {
        let execution = crate::clock::current();
        let backends = StorageBackends::in_memory(catalog.clone(), execution.clone())?;
        Ok(Self {
            engine: EngineManager::with_catalog_execution(catalog, backends, settings, execution)?,
        })
    }

    pub fn new(backends: StorageBackends, settings: StorageSettings) -> Result<Self> {
        Self::with_catalog(ObjectCatalog::new(), backends, settings)
    }
    pub fn with_catalog(catalog: ObjectCatalog, backends: StorageBackends, settings: StorageSettings) -> Result<Self> {
        let execution = crate::clock::current();
        Ok(Self {
            engine: EngineManager::with_catalog_execution(catalog, backends, settings, execution)?,
        })
    }
}

impl StorageManager {
    pub(crate) fn claim_owner(&self, owner: Arc<()>) -> Result<()> {
        self.engine.claim_owner(owner).map_err(Into::into)
    }
    #[tracing::instrument(target = "harmos::storage", name = "storage.acquire", level = "debug", skip_all)]
    pub(crate) async fn acquire(&self) -> Result<()> {
        self.engine.acquire().await.map_err(Into::into)
    }
    #[tracing::instrument(target = "harmos::storage", name = "storage.close", level = "debug", skip_all)]
    pub async fn close(&self) -> Result<()> {
        self.engine.close().await.map_err(Into::into)
    }

    pub(crate) fn engine(&self) -> EngineManager {
        self.engine.clone()
    }

    pub(crate) fn same_instance(&self, other: &StorageManager) -> bool {
        self.engine.same_instance(&other.engine)
    }
    #[tracing::instrument(target = "harmos::storage", name = "storage.start", level = "debug", skip_all)]
    pub async fn start<A: Application>(&self, runtime: &Runtime<A>) -> Result<()> {
        runtime.start_storage(self.clone()).await
    }
    #[tracing::instrument(target = "harmos::storage", name = "storage.start_on", level = "debug", skip_all)]
    pub(crate) async fn start_on<A: Application>(&self, operations: &Manager) -> Result<()> {
        self.engine
            .start_with(operations.resident_identity(), |worker| {
                let lease = operations
                    .start_service::<A>(Once::new(ServiceDefinition::new("harmos-storage"), Worker(worker)), true, false)
                    .map_err(|error| StorageError::backend("start storage worker", BackendFailure::Permanent, error))?;
                Ok(Box::new(Owner { leases: Mutex::new(vec![lease]) }))
            })
            .await
            .map_err(Into::into)
    }
}

impl<A: Application> Resident<A> for Worker {
    type Error = Error;
    async fn cleanup(&mut self) -> Result<()> {
        self.0.cleanup().await.map_err(Into::into)
    }
    async fn run(&mut self, mut context: ServiceContext<A>) -> StdResult<(), Error> {
        context.ready();
        context.until_stopping(self.0.run()).await.unwrap_or(Ok(())).map_err(|error| error.into())
    }
}

impl WorkerOwner for Owner {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let receipts: Vec<_> = self.leases.lock().unwrap().iter_mut().map(Lease::retire).collect();
            let mut failure = None;
            for receipt in receipts {
                if let Err(error) = receipt.await {
                    failure.get_or_insert_with(|| StorageError::backend("stop storage worker", BackendFailure::Permanent, error));
                }
            }
            failure.map_or(Ok(()), Err)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
