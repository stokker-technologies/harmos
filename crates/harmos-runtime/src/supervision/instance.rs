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
use std::sync::{Arc, Mutex, Once, Weak};

// Third Party Imports
use tokio::sync::watch;
use tokio::time::Instant;
use tracing::Instrument;

// Local Imports
use super::cleanup::{Cleanup, Finalize, Resource};
use super::executor;
use crate::Error;
use crate::supervision::executor::InContext;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Status {
    phase: Phase,
    outcome: Option<Outcome>,
    cancellation_requested: bool,
    dependents: usize,
    cleanup_pending: bool,
    cleanup_failure: Option<Arc<Error>>,
    attempt: usize,
    started_at: Option<Instant>,
    settled_at: Option<Instant>,
}

pub(crate) struct Instance {
    dispatcher: tracing::Dispatch,
    trace: Mutex<Option<tracing::Span>>,
    status: watch::Sender<Status>,
    // Resident instances end Stopped; request-scoped ones end Settled.
    terminal: Phase,
    finished: Once,
    on_settled: Option<Box<dyn Fn() + Send + Sync>>,
    cancellation: watch::Sender<bool>,
    children: Mutex<Vec<Weak<Instance>>>,
    retained: Mutex<Option<Arc<dyn Settlement>>>,
    providers: Mutex<Vec<Dependency>>,
}

struct Dependency {
    provider: Arc<Instance>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Initializing,
    Running,
    RetryWaiting,
    Restarting,
    Stopping,
    Finalizing,
    Stopped,
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Unavailable,
}

/* === Traits === */

trait Settlement: Send + Sync {
    fn settle(&self) -> Finalize<'_>;
}

/* === Implementations === */

impl Status {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }

    pub fn dependents(&self) -> usize {
        self.dependents
    }

    pub fn cleanup_pending(&self) -> bool {
        self.cleanup_pending
    }

    pub fn cleanup_failure(&self) -> Option<&Arc<Error>> {
        self.cleanup_failure.as_ref()
    }

    pub fn attempt(&self) -> usize {
        self.attempt
    }

    pub fn started_at(&self) -> Option<Instant> {
        self.started_at
    }

    pub fn settled_at(&self) -> Option<Instant> {
        self.settled_at
    }

    // Project onto the status every host binding reports.
    pub fn portable(&self) -> harmos_domain::operations::instance::Status {
        use harmos_domain::operations::instance;
        let phase = match self.phase {
            Phase::Queued => instance::Phase::Queued,
            Phase::Initializing => instance::Phase::Initializing,
            Phase::Running => instance::Phase::Running,
            Phase::RetryWaiting => instance::Phase::RetryWaiting,
            Phase::Restarting => instance::Phase::Restarting,
            Phase::Stopping => instance::Phase::Stopping,
            Phase::Finalizing => instance::Phase::CleaningUp,
            Phase::Stopped => instance::Phase::Stopped,
            Phase::Settled => instance::Phase::Settled,
        };
        let outcome = self.outcome.map(|outcome| match outcome {
            Outcome::Succeeded => instance::Outcome::Succeeded,
            Outcome::Failed => instance::Outcome::Failed,
            Outcome::Cancelled => instance::Outcome::Cancelled,
            Outcome::TimedOut => instance::Outcome::TimedOut,
            Outcome::Unavailable => instance::Outcome::Unavailable,
        });
        instance::Status {
            phase,
            outcome,
            cancellation_requested: self.cancellation_requested,
            attempt: u32::try_from(self.attempt).unwrap_or(u32::MAX),
        }
    }
}

impl Phase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Settled)
    }
}

impl Instance {
    pub(crate) fn queued(cancellation: watch::Sender<bool>) -> Self {
        let status = Status {
            phase: Phase::Queued,
            outcome: None,
            cancellation_requested: false,
            dependents: 0,
            cleanup_pending: false,
            cleanup_failure: None,
            attempt: 0,
            started_at: None,
            settled_at: None,
        };
        Self {
            dispatcher: tracing::dispatcher::get_default(Clone::clone),
            trace: Mutex::new(Some(
                tracing::debug_span!(target: "harmos::operations", "operations.instance", instance_id = tracing::field::Empty, operation = tracing::field::Empty, generation = tracing::field::Empty, outcome = tracing::field::Empty, settled = false),
            )),
            status: watch::channel(status).0,
            terminal: Phase::Settled,
            finished: Once::new(),
            on_settled: None,
            cancellation,
            children: Mutex::new(Vec::new()),
            retained: Mutex::new(None),
            providers: Mutex::new(Vec::new()),
        }
    }

    // A resident instance runs until stopped and ends in `Stopped`.
    pub(crate) fn resident(mut self) -> Self {
        self.terminal = Phase::Stopped;
        self
    }

    pub(crate) fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    pub(crate) fn trace(&self) -> tracing::Span {
        self.trace.lock().expect("instance trace lock is healthy").clone().unwrap_or_else(tracing::Span::none)
    }

    pub(crate) fn settlement(&self) -> watch::Receiver<Status> {
        self.status.subscribe()
    }

    pub(crate) fn transition(&self, phase: Phase) {
        tracing::dispatcher::with_default(&self.dispatcher, || {
            tracing::debug!(target: "harmos::operations", parent: &self.trace(), ?phase, "instance transition");
        });
        assert!(
            !matches!(phase, Phase::Queued | Phase::Stopped | Phase::Settled),
            "admission and physical settlement own terminal transitions"
        );
        let now = executor::now();
        self.status.send_modify(|status| {
            if status.phase.is_terminal() {
                return;
            }
            if status.cancellation_requested
                && matches!(phase, Phase::Initializing | Phase::Running | Phase::RetryWaiting | Phase::Restarting)
            {
                return;
            }
            status.phase = phase;
            if phase == Phase::Initializing {
                status.started_at.get_or_insert(now);
            }
            status.cleanup_pending = matches!(phase, Phase::Stopping | Phase::Finalizing);
        });
    }

    pub(crate) fn attempt(&self, attempt: usize) -> bool {
        self.begin_attempt(attempt, Phase::Running)
    }

    pub(crate) fn initialize_attempt(&self, attempt: usize) -> bool {
        self.begin_attempt(attempt, Phase::Initializing)
    }

    fn begin_attempt(&self, attempt: usize, phase: Phase) -> bool {
        tracing::dispatcher::with_default(&self.dispatcher, || {
            tracing::debug!(target: "harmos::operations", parent: &self.trace(), attempt, ?phase, "instance attempt");
        });
        let now = executor::now();
        let mut admitted = false;
        self.status.send_modify(|status| {
            if status.cancellation_requested || status.phase.is_terminal() {
                return;
            }
            status.phase = phase;
            status.cleanup_pending = false;
            status.attempt = attempt;
            status.started_at.get_or_insert(now);
            admitted = true;
        });
        admitted
    }

    pub(crate) fn complete(&self, outcome: Outcome) {
        tracing::dispatcher::with_default(&self.dispatcher, || {
            self.trace().record("outcome", tracing::field::debug(outcome));
        });
        self.status.send_modify(|status| {
            if !status.phase.is_terminal() {
                status.outcome = Some(outcome);
            }
        });
    }

    pub(crate) fn on_settled(mut self, notify: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_settled = Some(Box::new(notify));
        self
    }

    pub(crate) fn finished(&self) {
        self.finished.call_once(|| {
            let providers = std::mem::take(&mut *self.providers.lock().expect("instance providers are healthy"));
            drop(providers);
            if let Some(notify) = &self.on_settled {
                notify();
            }
            let now = executor::now();
            let mut failure = None;
            self.status.send_modify(|status| {
                status.phase = self.terminal;
                status.cleanup_pending = false;
                failure = status.cleanup_failure.take();
                status.settled_at = Some(now);
            });
            drop(failure);
            let trace = self.trace.lock().expect("instance trace lock is healthy").take();
            tracing::dispatcher::with_default(&self.dispatcher, || {
                if let Some(trace) = trace {
                    trace.record("settled", true);
                }
            });
        });
    }

    pub(crate) fn interrupted(&self, failure: Arc<Error>) {
        let mut previous = None;
        self.status.send_modify(|status| {
            if !status.phase.is_terminal() {
                status.outcome.get_or_insert(Outcome::Unavailable);
                status.cleanup_pending = true;
                previous = status.cleanup_failure.replace(failure);
            }
        });
        drop(previous);
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.status.borrow().phase.is_terminal()
    }

    pub(crate) fn track(&self, child: &Arc<Self>) {
        let mut children = self.children.lock().expect("instance child lock is healthy");
        children.retain(|child| child.upgrade().is_some_and(|child| !child.is_settled()));
        children.push(Arc::downgrade(child));
        drop(children);
        if self.status().cancellation_requested {
            child.cancel();
        }
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn depend_on(self: &Arc<Self>, provider: &Arc<Self>, retirement: bool) -> crate::Result<()> {
        if Arc::ptr_eq(self, provider) {
            return Err(Error::InvalidInput {
                field: "physical provider",
                reason: "an instance cannot retain itself",
            });
        }
        let mut providers = self.providers.lock().expect("instance providers are healthy");
        if providers.iter().any(|held| Arc::ptr_eq(&held.provider, provider)) {
            return Ok(());
        }
        let mut admitted = false;
        let mut stopping_control = false;
        provider.status.send_modify(|status| {
            if status.phase.is_terminal() {
                return;
            }
            if status.cancellation_requested {
                if !retirement {
                    return;
                }
                if status.dependents == 0 {
                    // The Existing Physical Stop Owner Drives Its Own SDK Controls.
                    stopping_control = true;
                    return;
                }
            }
            if let Some(count) = status.dependents.checked_add(1) {
                status.dependents = count;
                admitted = true;
            }
        });
        if stopping_control {
            return Ok(());
        }
        if !admitted {
            return Err(Error::Closing { domain: "physical provider" });
        }
        providers.push(Dependency { provider: provider.clone() });
        drop(providers);
        if !retirement {
            provider.track(self);
        }
        Ok(())
    }

    pub(crate) fn retain<R: Resource>(&self, resource: R, failure: Arc<Error>) {
        let mut retained = self.retained.lock().expect("instance cleanup lock is healthy");
        assert!(retained.is_none(), "one instance has one physical cleanup owner");
        *retained = Some(Arc::new(Cleanup::new(resource)));
        self.status.send_modify(|status| {
            if status.phase != Phase::Stopping {
                status.phase = Phase::Finalizing;
            }
            status.cleanup_pending = true;
            status.cleanup_failure = Some(failure);
        });
    }

    pub(crate) fn settle_dependents(&self) -> Finalize<'_> {
        Box::pin(async move {
            let children: Vec<_> = self.children.lock().expect("instance child lock is healthy").iter().filter_map(Weak::upgrade).collect();
            let pending = children.into_iter().map(|child| async move { child.settle().await });
            super::settle_all(pending).await
        })
    }

    pub(crate) fn settle(&self) -> Finalize<'_> {
        Box::pin(
            async move {
                let mut changed = self.settlement();
                loop {
                    changed.borrow_and_update();
                    self.settle_dependents().await?;
                    let retained = self.retained.lock().expect("instance cleanup lock is healthy").clone();
                    if let Some(retained) = retained {
                        if let Err(failure) = retained.settle().await {
                            let mut previous = None;
                            self.status.send_modify(|status| {
                                if !status.phase.is_terminal() {
                                    previous = status.cleanup_failure.replace(Arc::clone(&failure));
                                }
                            });
                            drop(previous);
                            return Err(failure);
                        }
                        self.finished();
                        return Ok(());
                    }
                    let status = self.status();
                    if status.phase.is_terminal() {
                        return Ok(());
                    }
                    if let Some(failure) = status.cleanup_failure {
                        return Err(failure);
                    }
                    if changed.changed().await.is_err() {
                        return Err(Arc::new(Error::Closing { domain: "instance settlement" }));
                    }
                }
            }
            .instrument(self.trace())
            .in_context(self.dispatcher.clone()),
        )
    }

    pub(crate) fn cancel(&self) {
        tracing::dispatcher::with_default(&self.dispatcher, || {
            tracing::debug!(target: "harmos::operations", parent: &self.trace(), "instance cancellation requested");
        });
        let mut interrupt = false;
        self.status.send_modify(|status| {
            interrupt = status.dependents == 0;
            status.cancellation_requested = true;
            if !status.phase.is_terminal() {
                status.cleanup_pending = true;
            }
            if !matches!(status.phase, Phase::Finalizing) && !status.phase.is_terminal() {
                status.phase = Phase::Stopping;
            }
        });
        if interrupt {
            self.cancellation.send_replace(true);
        }
        let children: Vec<_> = self.children.lock().expect("instance child lock is healthy").iter().filter_map(Weak::upgrade).collect();
        for child in children {
            child.cancel();
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let span = self.trace.get_mut().expect("instance trace lock is healthy").take();
        tracing::dispatcher::with_default(&self.dispatcher, || drop(span));
    }
}

impl<R: Resource> Settlement for Cleanup<R> {
    fn settle(&self) -> Finalize<'_> {
        Box::pin(self.settle())
    }
}

impl Drop for Dependency {
    fn drop(&mut self) {
        let mut interrupt = false;
        self.provider.status.send_modify(|status| {
            status.dependents = status.dependents.checked_sub(1).expect("each provider hold releases once");
            interrupt = status.cancellation_requested && status.dependents == 0;
        });
        if interrupt {
            self.provider.cancellation.send_replace(true);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
