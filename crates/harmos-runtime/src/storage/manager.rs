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

pub use harmos_storage::ErasedCatalog;
pub(crate) use harmos_storage::encoded_size;

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, Mutex};

// Third Party Imports

// Local Imports
use super::{StorageBackends, StorageSettings};
use crate::operations::{Manager, resident::Lease};
use crate::services::{Once, Resident, ServiceContext, ServiceDefinition};
use crate::supervision::executor;
use crate::{Application, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct StorageManager<C = ()> {
    engine: harmos_storage::StorageManager<C>,
}

struct Worker(harmos_storage::Worker);

struct Owner {
    leases: Mutex<Vec<Lease>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> Clone for StorageManager<C> {
    fn clone(&self) -> Self {
        Self { engine: self.engine.clone() }
    }
}

impl<C> std::ops::Deref for StorageManager<C> {
    type Target = harmos_storage::StorageManager<C>;
    fn deref(&self) -> &Self::Target {
        &self.engine
    }
}

impl<C: super::ObjectCatalog> StorageManager<C> {
    #[tracing::instrument(target = "harmos::storage", name = "storage.new", level = "debug", skip_all)]
    pub fn new(backends: StorageBackends, settings: StorageSettings) -> Result<Self> {
        let execution = harmos_storage::execution::Execution::new(executor::now, |duration| Box::pin(executor::sleep(duration)));
        Ok(Self {
            engine: harmos_storage::StorageManager::with_execution(backends, settings, execution)?,
        })
    }
}

impl<C> StorageManager<C> {
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

    pub(crate) fn engine_erased(&self) -> harmos_storage::StorageManager<ErasedCatalog> {
        self.engine.erased()
    }
    pub(crate) fn erased(&self) -> StorageManager<ErasedCatalog> {
        StorageManager { engine: self.engine.erased() }
    }
    pub(crate) fn same_instance<D>(&self, other: &StorageManager<D>) -> bool {
        self.engine.same_instance(&other.engine)
    }
    #[tracing::instrument(target = "harmos::storage", name = "storage.start", level = "debug", skip_all)]
    pub async fn start<A: Application>(&self, runtime: &crate::Runtime<A>) -> Result<()> {
        self.start_on::<A>(&runtime.operations).await
    }
    #[tracing::instrument(target = "harmos::storage", name = "storage.start_on", level = "debug", skip_all)]
    pub(crate) async fn start_on<A: Application>(&self, operations: &Manager) -> Result<()> {
        self.engine
            .start_with(operations.resident_identity(), |worker| {
                let lease = operations
                    .start_service::<A>(Once::new(ServiceDefinition::new("harmos-storage"), Worker(worker)), true, false)
                    .map_err(|error| {
                        harmos_storage::Error::backend("start storage worker", harmos_storage::BackendFailure::Permanent, error)
                    })?;
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
    async fn run(&mut self, mut context: ServiceContext<A>) -> std::result::Result<(), Error> {
        context.ready();
        context.until_stopping(self.0.run()).await.unwrap_or(Ok(())).map_err(|error| error.into())
    }
}

impl harmos_storage::WorkerOwner for Owner {
    fn stop(&self) -> harmos_storage::StorageFuture<'_, ()> {
        Box::pin(async move {
            let receipts: Vec<_> = self.leases.lock().unwrap().iter_mut().map(Lease::retire).collect();
            let mut failure = None;
            for receipt in receipts {
                if let Err(error) = receipt.await {
                    failure.get_or_insert_with(|| {
                        harmos_storage::Error::backend("stop storage worker", harmos_storage::BackendFailure::Permanent, error)
                    });
                }
            }
            failure.map_or(Ok(()), Err)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
