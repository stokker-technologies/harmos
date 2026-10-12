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
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, Weak};
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
use tokio::task::JoinHandle;
// Local Imports
use super::{Handle, Participants, Runtime, STOP_GRACE, Stopping};
use crate::storage::StorageManager;
use crate::supervision::cleanup::{Cleanup, Finalize, Resource};
use crate::{Application, Error, Position, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(super) type Outcome = std::result::Result<Position, Failure>;

/* === Structs === */
pub(super) struct Shutdown<A: Application> {
    handles: Handle<A>,
    participants: Participants,
    journal_handles: Weak<()>,
    notices: watch::Sender<Option<String>>,
    state: Drain,
    outcome: Arc<OnceLock<Outcome>>,
    progress: watch::Sender<&'static str>,
}

#[derive(Clone)]
pub(super) struct Failure {
    domain: &'static str,
    source: Arc<dyn std::error::Error + Send + Sync>,
}

/* === Enums === */
enum Drain {
    Running(JoinHandle<Position>),
    Drained(Outcome),
}

/* === Traits === */

/* === Implementations === */
impl<A: Application> Runtime<A> {
    pub async fn start_storage<C>(&self, storage: StorageManager<C>) -> Result<()> {
        let storage = storage.erased();
        let storage = {
            let _stopping = self.stopping.lock().expect("runtime transition is healthy");
            if self.closing.load(Ordering::Acquire) {
                return Err(Error::Closing { domain: "runtime" });
            }
            if let Some(current) = self.storage.get()
                && !current.same_instance(&storage)
            {
                return Err(Error::AlreadyRegistered {
                    domain: "runtime storage",
                    name: "primary".to_owned(),
                });
            }
            storage.claim_owner(self.services.identity())?;
            self.storage.get_or_init(|| storage)
        };
        storage.start(&self.services).await
    }

    pub fn storage(&self) -> Option<&StorageManager<crate::storage::ErasedCatalog>> {
        self.storage.get()
    }

    pub async fn stop_with_timeout(&self, timeout: Duration) -> Result<Position> {
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout)
            .filter(|_| !timeout.is_zero())
            .ok_or(Error::InvalidInput {
                field: "runtime shutdown timeout",
                reason: "must be positive and representable",
            })?;
        match tokio::time::timeout_at(deadline, self.stop()).await {
            Ok(result) => result,
            Err(_) => {
                let stopping = self.stopping.lock().expect("runtime transition is healthy");
                let phase = match &*stopping {
                    Stopping::Running(_) => "admission",
                    Stopping::Closing { progress, .. } => *progress.borrow(),
                };
                tracing::error!(
                    phase,
                    ?timeout,
                    "runtime shutdown budget exhausted; original cleanup retained"
                );
                Err(Error::ShutdownTimeout { phase })
            }
        }
    }

    pub async fn stop(&self) -> Result<Position> {
        self.closing.store(true, Ordering::Release);
        #[cfg(feature = "grpc")]
        self.grpc.close_admission();
        // Notify Residents Before Cancelling Their In-Flight Calls
        self.services.close_admission();
        self.operations.close_admission();
        self.observers.close_admission();
        #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
        {
            self.records.stop_listeners();
            self.streams.stop_listeners();
        }
        let (cleanup, completed) = {
            let mut stopping = self.stopping.lock().expect("runtime transition is healthy");
            if let Stopping::Running(supervisor) = &mut *stopping {
                let outcome = Arc::new(OnceLock::new());
                let progress = watch::channel("producers and listeners").0;
                let original = Shutdown {
                    handles: self.handle(),
                    participants: self.participants.clone(),
                    journal_handles: self.journal_handles.clone(),
                    notices: self.notices.clone(),
                    state: Drain::Running(supervisor.take().expect("runtime owns its supervisor")),
                    outcome: outcome.clone(),
                    progress: progress.clone(),
                };
                *stopping = Stopping::Closing {
                    cleanup: Arc::new(Cleanup::new(original)),
                    outcome,
                    progress,
                };
            }
            let Stopping::Closing {
                cleanup, outcome, ..
            } = &*stopping
            else {
                unreachable!("shutdown retains its driver before awaiting it");
            };
            (cleanup.clone(), outcome.clone())
        };
        cleanup.settle().await.map_err(|source| {
            Arc::try_unwrap(source).unwrap_or_else(|source| Error::TaskFailed {
                domain: "runtime cleanup",
                source,
            })
        })?;
        outcome(
            completed
                .get()
                .expect("physical shutdown published its outcome"),
        )
    }
}

impl<A: Application> Resource for Shutdown<A> {
    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(async move { self.finish().await.map_err(Arc::new) })
    }
}

impl<A: Application> Shutdown<A> {
    async fn finish(&mut self) -> Result<()> {
        if let Drain::Running(supervisor) = &mut self.state {
            self.progress.send_replace("observers and listeners");
            let observers = self.handles.observers.shutdown();
            #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
            let (observers, records, streams) = tokio::join!(
                observers,
                self.handles.records.shutdown_listeners(),
                self.handles.streams.shutdown_listeners(),
            );
            #[cfg(not(any(feature = "load", feature = "guest-host", feature = "sidecar-host")))]
            let observers = observers.await;
            if self.handles.observers.cleanup_pending() {
                return Err(observers.err().unwrap_or(Error::NotReady {
                    domain: "observer cleanup",
                }));
            }
            #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
            records?;
            #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
            streams?;
            self.progress.send_replace("application services");
            let services = self.handles.services.shutdown_application().await;
            if self.handles.services.cleanup_pending(false) {
                return Err(services.err().unwrap_or(Error::NotReady {
                    domain: "service cleanup",
                }));
            }
            // Original SDK Shutdown Keeps Its Dispatcher Until Application Cleanup Settles.
            self.progress.send_replace("operations");
            self.handles.operations.shutdown().await?;
            self.progress.send_replace("participants");
            self.participants.stop().await;
            self.handles.state.seal();
            self.progress.send_replace("journal");
            let mut settled = joined(supervisor, &self.journal_handles, &self.notices).await;
            if let Err(source) = observers
                && settled.is_ok()
            {
                settled = Err(Failure {
                    domain: "observer",
                    source: Arc::new(source),
                });
            }
            if let Err(source) = services
                && settled.is_ok()
            {
                settled = Err(Failure {
                    domain: "service",
                    source: Arc::new(source),
                });
            }
            self.handles.streams.close();
            self.state = Drain::Drained(settled);
        }
        let Drain::Drained(settled) = &self.state else {
            unreachable!("the journal is drained before storage closes");
        };
        let mut settled = settled.clone();
        if let Some(storage) = self.handles.storage() {
            self.progress.send_replace("storage");
            storage.close().await?;
        }
        self.progress.send_replace("infrastructure services");
        let services = self.handles.services.shutdown().await;
        if self.handles.services.cleanup_pending(true) {
            return Err(services.err().unwrap_or(Error::NotReady {
                domain: "service cleanup",
            }));
        }
        if let Err(source) = services
            && settled.is_ok()
        {
            settled = Err(Failure {
                domain: "service",
                source: Arc::new(source),
            });
        }
        self.outcome
            .set(settled)
            .unwrap_or_else(|_| unreachable!("shutdown publishes its outcome once"));
        Ok(())
    }
}

impl<A: Application> Handle<A> {
    pub fn storage(&self) -> Option<&StorageManager<crate::storage::ErasedCatalog>> {
        self.storage.get()
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn joined(
    supervisor: &mut JoinHandle<Position>,
    journal_handles: &Weak<()>,
    notices: &watch::Sender<Option<String>>,
) -> Outcome {
    let mut grace = Box::pin(tokio::time::sleep(STOP_GRACE));
    let joined = std::future::poll_fn(|context| {
        if let Poll::Ready(settled) = Pin::new(&mut *supervisor).poll(context) {
            return Poll::Ready(Some(settled));
        }
        if grace.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }

        Poll::Pending
    })
    .await;

    match joined {
        Some(settled) => settled,
        None => {
            let live = journal_handles.strong_count();
            notices.send_replace(Some(format!(
                "the journal stop is still draining; {live} StateManager clone{} outstanding, none \
                     of which holds it open",
                if live == 1 { "" } else { "s" }
            )));

            supervisor.await
        }
    }
    .map_err(|source| Failure {
        domain: "journal supervisor",
        source: Arc::new(source),
    })
}

fn outcome(settled: &Outcome) -> Result<Position> {
    match settled {
        Ok(position) => Ok(*position),
        Err(failure) => Err(Error::TaskFailed {
            domain: failure.domain,
            source: failure.source.clone(),
        }),
    }
}

/* === Tests === */
