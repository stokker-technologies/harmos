/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// One sidecar connection, run as a resident service instance the plugin
// manager owns. The instance runs the connection once and holds its cleanup
// until the plugin manager stops it; the manager polls the connection's
// outcome and, during settlement, the instance's cleanup inline with its own
// lifecycle steps.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Wake, Waker};
// Third Party Imports
use futures_util::FutureExt;
use tokio::sync::watch;
// Local Imports
use super::{Notice, Supervised};
use crate::operations::resident::Lease;
use crate::operations::{Manager, RestartPolicy};
use crate::services::{Once, Resident, ServiceContext, ServiceDefinition};
use crate::{Application, Result};
use harmos_sidecar::Attach;

/* === Constants === */

static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

type Outcome = std::result::Result<(), String>;
type Running = Pin<Box<dyn Future<Output = Outcome> + Send>>;
type Ending = Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send>>;
type Owner<A, S, H> = std::marker::PhantomData<fn() -> (A, S, H)>;

/* === Structs === */

pub(super) struct Physical<A: Application, S, H> {
    original: Arc<Mutex<Original>>,
    marker: Owner<A, S, H>,
}

pub(super) struct Polling<A: Application, S, H> {
    original: Weak<Mutex<Original>>,
    marker: Owner<A, S, H>,
}

struct Original {
    signals: Arc<Signals>,
    lease: Option<Lease>,
    end: watch::Sender<bool>,
    running_busy: bool,
    ending_busy: bool,
    cancelled: bool,
    running: Option<Running>,
    ending: Option<Ending>,
    settled: bool,
    ending_failure: Option<String>,
    outcome: Option<Outcome>,
}

#[derive(Default)]
struct Signals {
    revision: AtomicU64,
    waiting: Mutex<Vec<Waker>>,
}

// The resident the connection's service instance runs: the connection once,
// then nothing until the plugin manager stops the instance.
struct Connection<A: Application, S, H> {
    supervised: Supervised<A, S, H>,
    outcome: watch::Sender<Option<Outcome>>,
    // Set by the plugin manager. Instance cancellation waits for every
    // dependent observer, which retires only in this connection's cleanup.
    end: watch::Receiver<bool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Signals {
    fn register(&self, waker: &Waker) {
        let mut waiting = self.waiting.lock().expect("physical wake registry is healthy");
        if !waiting.iter().any(|current| current.will_wake(waker)) {
            waiting.push(waker.clone());
        }
    }

    fn notify(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
        let waiting = std::mem::take(&mut *self.waiting.lock().expect("physical wake registry is healthy"));
        for waker in waiting {
            waker.wake();
        }
    }
}

impl Wake for Signals {
    fn wake(self: Arc<Self>) {
        self.notify();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify();
    }
}

impl<A: Application, S, H> Clone for Polling<A, S, H> {
    fn clone(&self) -> Self {
        Self { original: self.original.clone(), marker: std::marker::PhantomData }
    }
}

impl<A, S, H> Resident<A> for Connection<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    type Error = std::io::Error;

    async fn run(&mut self, context: ServiceContext<A>) -> std::result::Result<(), Self::Error> {
        let mut stopping = context.clone();
        let mut end = self.end.clone();
        // Ending Drops The Connection Attempt; Its Cleanup Runs Afterwards
        let running = AssertUnwindSafe(self.supervised.run(context)).catch_unwind();
        let ended = tokio::select! {
            ended = running => Some(ended.unwrap_or_else(|_| {
                Err(std::io::Error::other("native physical attempt panicked"))
            })),
            _ = end.wait_for(|ended| *ended) => None,
            () = stopping.stopping() => None,
        };
        if let Some(outcome) = ended {
            self.outcome.send_replace(Some(outcome.map_err(|error| error.to_string())));
            tokio::select! {
                _ = end.wait_for(|ended| *ended) => {}
                () = stopping.stopping() => {}
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> std::result::Result<(), Self::Error> {
        self.supervised.stop().await
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        self.supervised.cleanup().await
    }
}

impl<A, S, H> Physical<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    // Start the connection as a service instance; the plugin manager restarts
    // plugins, so the instance itself never restarts.
    pub(super) fn new(resident: Supervised<A, S, H>, operations: &Manager) -> Result<Self> {
        let number = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
        let definition = ServiceDefinition {
            id: format!("harmos.sidecar.{}.{number}", resident.attach.owner()),
            policy: crate::ServicePolicy { restart: RestartPolicy::Never, ..crate::services::POLICY },
        };
        let (outcome, mut finished) = watch::channel(None);
        let (end, ending) = watch::channel(false);
        let connection = Connection { supervised: resident, outcome, end: ending };
        let lease = operations.start_service::<A>(Once::new(definition, connection), true, false)?;
        let running: Running = Box::pin(async move {
            match finished.wait_for(Option::is_some).await {
                Ok(outcome) => outcome.clone().expect("a finished connection has an outcome"),
                Err(_) => Err("native physical attempt was interrupted".into()),
            }
        });
        Ok(Self {
            original: Arc::new(Mutex::new(Original {
                signals: Arc::default(),
                lease: Some(lease),
                end,
                running_busy: false,
                ending_busy: false,
                cancelled: false,
                running: Some(running),
                ending: None,
                settled: false,
                ending_failure: None,
                outcome: None,
            })),
            marker: std::marker::PhantomData,
        })
    }

    pub(super) fn polling(&self) -> Polling<A, S, H> {
        Polling { original: Arc::downgrade(&self.original), marker: std::marker::PhantomData }
    }

    fn signals(&self, context: &Context<'_>) -> Arc<Signals> {
        let signals = self.original.lock().unwrap_or_else(|error| error.into_inner()).signals.clone();
        signals.register(context.waker());
        signals
    }

    pub(super) fn poll(&mut self, context: &mut Context<'_>) -> Poll<Outcome> {
        let signals = self.signals(context);
        let mut running = {
            let mut original = self.original.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(outcome) = &original.outcome {
                return Poll::Ready(outcome.clone());
            }
            if original.running_busy {
                return Poll::Pending;
            }
            let Some(running) = original.running.take() else {
                return Poll::Ready(Err("native physical attempt was interrupted".into()));
            };
            original.running_busy = true;
            running
        };
        let revision = signals.revision.load(Ordering::Acquire);
        let waker = Waker::from(signals.clone());
        let mut polling = Context::from_waker(&waker);
        let result = running.as_mut().poll(&mut polling);
        let mut original = self.original.lock().unwrap_or_else(|error| error.into_inner());
        original.running_busy = false;
        match result {
            Poll::Pending => {
                if !original.cancelled {
                    original.running = Some(running);
                }
                drop(original);
                if signals.revision.load(Ordering::Acquire) != revision {
                    signals.notify();
                }
                Poll::Pending
            }
            Poll::Ready(outcome) => {
                original.outcome = Some(outcome.clone());
                drop(original);
                drop(running);
                signals.notify();
                Poll::Ready(outcome)
            }
        }
    }

    // The plugin started: its connection instance runs.
    pub(super) fn ready(&self) {
        let original = self.original.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(lease) = &original.lease {
            lease.transition(crate::operations::OperationPhase::Running);
        }
    }

    pub(super) fn is_running(&self) -> bool {
        let original = self.original.lock().unwrap_or_else(|error| error.into_inner());
        original.running.is_some() || original.running_busy
    }

    // A sibling poller (dependency or progress) may have observed the attempt end first.
    pub(super) fn has_outcome(&self) -> bool {
        self.original.lock().unwrap_or_else(|error| error.into_inner()).outcome.is_some()
    }

    // Stop the connection's instance; its cleanup settles through `poll_ending`.
    pub(super) fn cancel(&mut self) {
        let signals = {
            let mut original = self.original.lock().unwrap_or_else(|error| error.into_inner());
            original.cancelled = true;
            original.running.take();
            original.end.send_replace(true);
            original.signals.clone()
        };
        signals.notify();
    }

    pub(super) fn retry_ending(&mut self) {
        self.original.lock().unwrap_or_else(|error| error.into_inner()).ending_failure.take();
    }

    pub(super) fn poll_ending(&mut self, context: &mut Context<'_>) -> Poll<std::result::Result<(), String>> {
        let signals = self.signals(context);
        let mut ending = {
            let mut original = self.original.lock().unwrap_or_else(|error| error.into_inner());
            if original.settled {
                return Poll::Ready(Ok(()));
            }
            if let Some(failure) = &original.ending_failure {
                return Poll::Ready(Err(failure.clone()));
            }
            if original.ending_busy || original.running_busy || original.running.is_some() {
                return Poll::Pending;
            }
            original.ending_busy = true;
            match original.ending.take() {
                Some(ending) => ending,
                None => {
                    original.end.send_replace(true);
                    let Some(lease) = original.lease.as_mut() else {
                        original.ending_busy = false;
                        original.settled = true;
                        return Poll::Ready(Ok(()));
                    };
                    let retiring = lease.retire();
                    Box::pin(async move { retiring.await.map_err(|error| error.to_string()) })
                }
            }
        };
        let revision = signals.revision.load(Ordering::Acquire);
        let waker = Waker::from(signals.clone());
        let mut polling = Context::from_waker(&waker);
        let result = ending.as_mut().poll(&mut polling);
        let mut original = self.original.lock().unwrap_or_else(|error| error.into_inner());
        original.ending_busy = false;
        match result {
            Poll::Pending => {
                original.ending = Some(ending);
                drop(original);
                if signals.revision.load(Ordering::Acquire) != revision {
                    signals.notify();
                }
                Poll::Pending
            }
            Poll::Ready(outcome) => {
                match &outcome {
                    Ok(()) => {
                        original.settled = true;
                        original.lease.take();
                    }
                    Err(failure) => original.ending_failure = Some(failure.clone()),
                }
                drop(original);
                drop(ending);
                signals.notify();
                Poll::Ready(outcome)
            }
        }
    }
}

impl<A, S, H> Polling<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync + 'static,
{
    pub(super) fn poll(&self, context: &mut Context<'_>) -> Poll<Outcome> {
        let Some(original) = self.original.upgrade() else {
            return Poll::Pending;
        };
        let mut physical = Physical::<A, S, H> { original, marker: std::marker::PhantomData };
        if physical.is_running() { physical.poll(context) } else { Poll::Pending }
    }

    pub(super) fn retry_ending(&self) {
        if let Some(original) = self.original.upgrade() {
            Physical::<A, S, H> { original, marker: std::marker::PhantomData }.retry_ending();
        }
    }

    pub(super) fn poll_ending(&self, context: &mut Context<'_>) -> Poll<std::result::Result<(), String>> {
        let Some(original) = self.original.upgrade() else {
            return Poll::Pending;
        };
        let mut physical = Physical::<A, S, H> { original, marker: std::marker::PhantomData };
        if physical.is_running() || physical.original.lock().unwrap_or_else(|error| error.into_inner()).settled {
            return Poll::Pending;
        }
        physical.poll_ending(context)
    }

    pub(super) fn same(&self, physical: &Physical<A, S, H>) -> bool {
        self.original.ptr_eq(&Arc::downgrade(&physical.original))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
