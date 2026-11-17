/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage-Only Assembly

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::result;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{BootstrapCleanup, BootstrapStatus, Builder, boot};
use crate::operations::{Lane, Manager, instances::Instances, lane::Lanes};
use crate::supervision::Task;
use crate::supervision::cleanup::{Cleanup, Finalize, Resource};
use crate::{Application, Error, Result, clock, storage};

use harmos_operations::execution::__private::ResourceSet;
use harmos_operations::storage::{SavedSnapshots, storage_catalog};
use harmos_storage::{self, StorageFuture, StorageManager, WorkerOwner};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A Storage Session Borrowing Assembly Until Its Operations Have Drained.
///
/// ```compile_fail,E0599
/// use harmos_runtime::runtime::StorageRuntime;
/// fn manager(session: &StorageRuntime<'_>) {
///     session.operations();
/// }
/// ```
pub struct StorageRuntime<'a> {
    operations: Manager,
    cleanup: boot::Cleanup,
    receipt: BootstrapCleanup,
    assembly: PhantomData<&'a mut ()>,
}

/// An Opening or Cleanup Failure Retaining Its Retryable Cleanup Receipt.
#[derive(thiserror::Error)]
#[error("{source}; settle the storage cleanup receipt before resuming assembly")]
pub struct StorageFailure {
    #[source]
    pub source: Arc<Error>,

    pub cleanup: BootstrapCleanup,
}

struct Session {
    storage: StorageManager,
    operations: Manager,
}

struct Worker(Task<result::Result<(), Arc<harmos_storage::Error>>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> Builder<A> {
    /// Open Only Storage; Close This Session Before Resuming Full Assembly.
    pub async fn storage_only(&mut self) -> Result<StorageRuntime<'_>> {
        clock::scope(self.clock.clone(), self.open_storage()).await
    }

    pub fn storage_cleanup(&self) -> Option<BootstrapCleanup> {
        self.storage_cleanup.clone()
    }

    async fn open_storage(&mut self) -> Result<StorageRuntime<'_>> {
        if self.storage_cleanup.as_ref().is_some_and(|cleanup| cleanup.status() != BootstrapStatus::Settled) {
            return Err(Error::NotReady {
                domain: "storage-only cleanup; retry and await storage_cleanup before reopening",
            });
        }
        if self.startup_storage.is_none() {
            let storage = storage::StorageManager::default_in_memory()?;
            self.storage(&storage);
        }
        let storage = self.startup_storage.as_ref().expect("storage is registered").engine().new_session()?;
        self.operation_settings.validate()?;
        let lanes = Lanes::freeze(self.lanes.clone()).map_err(|reason| Error::TaskFailed {
            domain: "configure storage-only lanes; supply valid lane settings",
            source: Arc::new(io::Error::other(reason)),
        })?;
        let mut resources = ResourceSet::new();
        resources.replace(Arc::new(storage.clone()));
        resources.replace(Arc::new(SavedSnapshots::new(storage.query_capacity())));
        let instances = Instances::start(lanes, Box::new(()), resources, &self.operation_settings);
        let operations = Manager::new::<A>(
            instances,
            &self.operation_settings,
            &self.grpc_plugins,
            BTreeSet::new(),
            Arc::new(()),
            self.schema_settings.clone(),
        );
        for middleware in &self.operation_middleware {
            operations.register_middleware(middleware.clone())?;
        }
        let owner = Arc::new(Cleanup::new(Session {
            storage: storage.clone(),
            operations: operations.clone(),
        }));
        let receipt = BootstrapCleanup::new();
        let cleanup = receipt.own(move || {
            let owner = owner.clone();
            Box::pin(async move { owner.settle().await.map_err(|source| Error::TaskFailed { domain: "storage-only cleanup", source }) })
        });
        self.storage_cleanup = Some(receipt.clone());
        let runtime = StorageRuntime {
            operations: operations.clone(),
            cleanup,
            receipt,
            assembly: PhantomData,
        };
        let opened: Result<()> = async {
            let registration = crate::operations::Registration::catalog(storage_catalog(), Lane::default())?;
            runtime.operations.register_native(registration)?;
            storage
                .start_with(Arc::new(()), |mut worker| {
                    let task = Task::spawn(move |_| async move {
                        worker.run().await.map_err(Arc::new)?;
                        worker.cleanup().await.map_err(Arc::new)
                    });
                    Ok(Box::new(Worker(task)))
                })
                .await
                .map_err(Error::from)
        }
        .await;
        if let Err(source) = opened {
            let _cleanup = runtime.close().await;
            return Err(Error::TaskFailed {
                domain: "open storage-only runtime",
                source: Arc::new(StorageFailure {
                    source: Arc::new(source),
                    cleanup: runtime.receipt.clone(),
                }),
            });
        }
        Ok(runtime)
    }
}

impl StorageRuntime<'_> {
    pub fn for_call(&self, context: &(impl crate::facade::CallContextSource + ?Sized)) -> crate::Call {
        crate::Call::new(self.operations.clone(), context)
    }

    /// Drain Admitted Operations and Close Both Backends Before Full Startup.
    pub async fn close(&self) -> Result<()> {
        self.cleanup.close().await.map_err(|source| Error::TaskFailed {
            domain: "close storage-only runtime",
            source: Arc::new(StorageFailure {
                source: Arc::new(source),
                cleanup: self.receipt.clone(),
            }),
        })
    }
}

impl Drop for StorageRuntime<'_> {
    fn drop(&mut self) {
        if matches!(self.receipt.status(), BootstrapStatus::Starting | BootstrapStatus::Cleaning) {
            self.receipt.cancelled(self.cleanup.clone());
        }
    }
}

impl fmt::Debug for StorageFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("StorageFailure").field("source", &self.source).field("cleanup", &self.cleanup.status()).finish()
    }
}

impl Resource for Session {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(async move {
            self.operations.shutdown().await.map_err(Arc::new)?;
            self.storage.close().await.map_err(|source| Arc::new(Error::from(source)))
        })
    }
}

impl WorkerOwner for Worker {
    fn stop(&self) -> StorageFuture<'_, ()> {
        Box::pin(async move {
            let result = self.0.settle().await.map_err(|source| {
                harmos_storage::Error::backend("join storage-only worker", harmos_storage::BackendFailure::Permanent, source)
            })?;
            result.as_ref().as_ref().map(|_| ()).map_err(|source| harmos_storage::Error::Persistence { source: source.clone() })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use crate::operations::NativeOperation;
    use crate::plugin::grpc::Load;
    use crate::state::Never;
    use crate::{Application, Lifecycle, Runtime, Setup, Startup};

    struct Minimal;

    impl Lifecycle for Minimal {
        type Config = ();
        type Error = Infallible;
        type Context = Startup<Self>;

        async fn prepare(_: (), _: &mut Setup<Self>) -> Result<Self, Infallible> {
            Ok(Self)
        }
    }

    impl Application for Minimal {
        type State = ();
        type Transaction = Never;
        type Record = ();
    }

    #[tokio::test]
    async fn storage_only_excludes_plugin_activation_until_full_startup() {
        // Admit Storage Without Plugin Activation
        let mut builder = Runtime::<Minimal>::builder(());
        let session = builder.storage_only().await.unwrap();
        assert!(session.operations.definition(Load::ID).is_err());
        assert!(session.operations.definitions().iter().all(|entry| !entry.id.as_str().starts_with("plugins.")));

        // Full Startup Registers the Plugin Operation
        session.close().await.unwrap();
        drop(session);
        let runtime = builder.start().await.unwrap();
        assert!(runtime.operations.definition(Load::ID).is_ok());
        runtime.shutdown().await.unwrap();
    }
}
