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

/* === Imports === */
// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::Runtime;
use crate::storage::StorageManager;
use crate::{Application, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type StartupFuture<A> = Pin<Box<dyn Future<Output = std::result::Result<Runtime<A>, super::Incompatible>> + Send>>;
type CleanupFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
type ReconcileFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
type InitializeFuture = Pin<Box<dyn Future<Output = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send>>;
type Initialize<A> = Box<dyn FnOnce(super::Handle<A>) -> InitializeFuture + Send>;
pub(super) type Reconciliation<A> =
    Box<dyn FnOnce(super::StateManager<A>, StorageManager<crate::storage::ErasedCatalog>) -> ReconcileFuture + Send>;

/* === Structs === */
pub(super) struct Initializer<A: Application> {
    pub(super) timeout: Duration,
    pub(super) initialize: Initialize<A>,
}

#[derive(Clone)]
pub(super) struct Cleanup {
    close: Arc<dyn Fn() -> CleanupFuture + Send + Sync>,
    receipt: Weak<CleanupState>,
}

pub struct Startup<A: Application> {
    future: Option<StartupFuture<A>>,
    cleanup: BootstrapCleanup,
}

#[derive(Clone)]
pub struct BootstrapCleanup {
    shared: Arc<CleanupState>,
}

struct CleanupState {
    retained: Mutex<Retained>,
    status: watch::Sender<BootstrapStatus>,
}

struct Retained {
    owner: Option<Cleanup>,
    failure: Option<Arc<crate::Error>>,
}

pub(super) struct Boot<A: Application> {
    pub(super) identity: Arc<()>,
    pub(super) storage: Option<StorageManager<crate::storage::ErasedCatalog>>,
    pub(super) runtime: Option<Runtime<A>>,
    pub(super) writer: Option<tokio::task::JoinHandle<()>>,
    pub(super) state: Option<super::StateManager<A>>,
    armed: bool,
    cleanup: BootstrapCleanup,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapStatus {
    Starting,
    Cleaning,
    Failed,
    Settled,
    HandedOff,
}

/* === Traits === */

/* === Implementations === */
impl<A: Application> Startup<A> {
    pub(super) fn new(builder: super::Builder<A>, storage: Option<StorageManager<crate::storage::ErasedCatalog>>) -> Self {
        let cleanup = BootstrapCleanup::new();
        let receipt = cleanup.clone();
        let future = Box::pin(async move {
            let outcome = builder.boot(storage.as_ref(), receipt.clone()).await;
            receipt.settle_unowned();
            outcome
        });
        Self { future: Some(future), cleanup }
    }

    pub fn cleanup(&self) -> BootstrapCleanup {
        self.cleanup.clone()
    }
}

impl<A: Application> Future for Startup<A> {
    type Output = std::result::Result<Runtime<A>, super::Incompatible>;
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let future = self.future.as_mut().expect("startup is polled before completion");
        let outcome = future.as_mut().poll(context);
        if outcome.is_ready() {
            self.future.take();
        }
        outcome
    }
}

impl<A: Application> Drop for Startup<A> {
    fn drop(&mut self) {
        // Drop Boot First So It Can Transfer Every Acquired Owner
        self.future.take();
        self.cleanup.settle_unowned();
    }
}

impl BootstrapCleanup {
    fn new() -> Self {
        let (status, _) = watch::channel(BootstrapStatus::Starting);
        Self { shared: Arc::new(CleanupState { retained: Mutex::new(Retained { owner: None, failure: None }), status }) }
    }

    pub fn status(&self) -> BootstrapStatus {
        *self.shared.status.borrow()
    }

    pub fn failure(&self) -> Option<Arc<crate::Error>> {
        self.shared.retained.lock().expect("bootstrap cleanup lock").failure.clone()
    }

    pub async fn wait(&self) -> Result<()> {
        let mut status = self.shared.status.subscribe();
        loop {
            let current = *status.borrow_and_update();
            match current {
                BootstrapStatus::Settled | BootstrapStatus::HandedOff => return Ok(()),
                BootstrapStatus::Failed => {
                    let source = {
                        let retained = self.shared.retained.lock().expect("bootstrap cleanup lock");
                        if self.status() != BootstrapStatus::Failed {
                            continue;
                        }
                        retained.failure.clone().expect("failed cleanup retains its cause")
                    };
                    return Err(crate::Error::TaskFailed { domain: "bootstrap cleanup", source });
                }
                BootstrapStatus::Starting | BootstrapStatus::Cleaning => {}
            }
            status.changed().await.expect("receipt retains its status owner");
        }
    }

    pub async fn retry(&self) -> Result<()> {
        let owner = {
            let mut retained = self.shared.retained.lock().expect("bootstrap cleanup lock");
            if self.status() == BootstrapStatus::Failed {
                let owner = retained.owner.clone().expect("failed cleanup owns resources");
                retained.failure = None;
                self.shared.status.send_replace(BootstrapStatus::Cleaning);
                Some(owner)
            } else {
                None
            }
        };
        if let Some(owner) = owner {
            self.run(owner);
        }
        self.wait().await
    }

    fn run(&self, owner: Cleanup) {
        tokio::spawn(async move {
            let _ = owner.close().await;
        });
    }

    pub(super) fn failed(&self, owner: Option<Cleanup>, failure: Arc<crate::Error>) {
        let mut retained = self.shared.retained.lock().expect("bootstrap cleanup lock");
        retained.owner = owner;
        retained.failure = Some(failure);
        self.shared.status.send_replace(BootstrapStatus::Failed);
    }

    fn cancelled(&self, owner: Cleanup) {
        {
            let mut retained = self.shared.retained.lock().expect("bootstrap cleanup lock");
            retained.owner = Some(owner.clone());
            self.shared.status.send_replace(BootstrapStatus::Cleaning);
        }
        self.run(owner);
    }

    fn settle_unowned(&self) {
        let retained = self.shared.retained.lock().expect("bootstrap cleanup lock");
        if retained.owner.is_none() && self.status() == BootstrapStatus::Starting {
            self.shared.status.send_replace(BootstrapStatus::Settled);
        }
    }
}

impl<A: Application> Initializer<A> {
    pub(super) fn new<F, Fut, E>(timeout: Duration, initialize: F) -> Self
    where
        F: FnOnce(super::Handle<A>) -> Fut + Send + 'static,
        Fut: Future<Output = std::result::Result<(), E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            timeout,
            initialize: Box::new(move |handle| Box::pin(async move { initialize(handle).await.map_err(|source| Box::new(source) as _) })),
        }
    }
}

impl Cleanup {
    pub(super) async fn close(&self) -> Result<()> {
        let outcome = (self.close)().await;
        let Some(receipt) = self.receipt.upgrade() else {
            return outcome;
        };
        let mut retained = receipt.retained.lock().expect("bootstrap cleanup lock");
        match outcome {
            Ok(()) => {
                retained.owner = None;
                retained.failure = None;
                receipt.status.send_replace(BootstrapStatus::Settled);
                Ok(())
            }
            Err(error) => {
                let source = Arc::new(error);
                retained.failure = Some(Arc::clone(&source));
                receipt.status.send_replace(BootstrapStatus::Failed);
                Err(crate::Error::TaskFailed { domain: "bootstrap cleanup", source })
            }
        }
    }
}

impl std::fmt::Debug for Cleanup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Cleanup").finish_non_exhaustive()
    }
}

impl<A: Application> Boot<A> {
    pub(super) fn new(storage: Option<&StorageManager<crate::storage::ErasedCatalog>>, cleanup: BootstrapCleanup) -> Result<Self> {
        let identity = Arc::new(());
        if let Some(storage) = storage {
            // Refusal must precede installing a cleanup owner: another runtime's
            // manager must never be closed by this failed opening attempt.
            storage.claim_owner(identity.clone())?;
        }

        Ok(Self { identity, storage: storage.cloned(), runtime: None, writer: None, state: None, armed: true, cleanup })
    }

    pub(super) async fn acquire(&self) -> Result<()> {
        if let Some(storage) = &self.storage {
            storage.acquire().await?;
        }
        Ok(())
    }

    pub(super) async fn close(&mut self) -> Result<()> {
        if self.runtime.is_none() {
            if let Some(state) = self.state.take() {
                state.seal();
            }
            if let Some(writer) = self.writer.as_mut() {
                let _ = writer.await;
            }
            self.writer.take();
        }
        let result = if let Some(runtime) = &self.runtime {
            runtime.stop().await.map(|_| ())
        } else if let Some(storage) = &self.storage {
            storage.close().await
        } else {
            Ok(())
        };
        // Operation finalization may refuse before the state is joined.
        // Keep the original runtime until success or transfer it to the error.
        self.armed = false;
        if result.is_ok() {
            self.runtime.take();
        }
        result
    }

    pub(super) fn retain_cleanup(&mut self) -> Option<Cleanup> {
        if let Some(runtime) = self.runtime.take() {
            let runtime = Arc::new(runtime);
            return Some(Cleanup {
                receipt: Arc::downgrade(&self.cleanup.shared),
                close: Arc::new(move || {
                    let runtime = Arc::clone(&runtime);
                    Box::pin(async move { runtime.stop().await.map(|_| ()) })
                }),
            });
        }
        self.storage.take().map(|storage| Cleanup {
            receipt: Arc::downgrade(&self.cleanup.shared),
            close: Arc::new(move || {
                let storage = storage.clone();
                Box::pin(async move { storage.close().await })
            }),
        })
    }

    pub(super) fn finish(mut self) -> Runtime<A> {
        self.armed = false;
        self.cleanup.shared.status.send_replace(BootstrapStatus::HandedOff);
        self.runtime.take().expect("successful boot owns a runtime")
    }
}

impl<A: Application> Drop for Boot<A> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let state = self.state.take();
        let writer = self.writer.take();
        let runtime = self.runtime.take();
        let storage = self.storage.take();
        if runtime.is_none() && storage.is_none() && writer.is_none() {
            return;
        }
        // Retain Partial Writer And StateManager Ownership Alongside The Full Runtime
        let writer = Arc::new(tokio::sync::Mutex::new(writer));
        let runtime = runtime.map(Arc::new);
        let owner = Cleanup {
            receipt: Arc::downgrade(&self.cleanup.shared),
            close: Arc::new(move || {
                let state = state.clone();
                let writer = Arc::clone(&writer);
                let runtime = runtime.clone();
                let storage = storage.clone();
                Box::pin(async move {
                    if runtime.is_none() {
                        if let Some(state) = state {
                            state.seal();
                        }
                        let mut writer = writer.lock().await;
                        if let Some(task) = writer.as_mut() {
                            let _ = task.await;
                        }
                        writer.take();
                    }
                    if let Some(runtime) = runtime {
                        runtime.stop().await.map(|_| ())
                    } else if let Some(storage) = storage {
                        storage.close().await
                    } else {
                        Ok(())
                    }
                })
            }),
        };
        self.cleanup.cancelled(owner);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
