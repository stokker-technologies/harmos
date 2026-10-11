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

// Local Imports
use super::cleanup::{Cleanup, Finalize, Resource};
use super::executor;
use crate::Error;

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
    status: watch::Sender<Status>,
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
    Stopping,
    Finalizing,
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
            status: watch::channel(status).0,
            finished: Once::new(),
            on_settled: None,
            cancellation,
            children: Mutex::new(Vec::new()),
            retained: Mutex::new(None),
            providers: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    pub(crate) fn settlement(&self) -> watch::Receiver<Status> {
        self.status.subscribe()
    }

    pub(crate) fn transition(&self, phase: Phase) {
        assert!(
            !matches!(phase, Phase::Queued | Phase::Settled),
            "admission and physical settlement own terminal transitions"
        );
        let now = executor::now();
        self.status.send_modify(|status| {
            if status.phase == Phase::Settled {
                return;
            }
            if status.cancellation_requested
                && matches!(
                    phase,
                    Phase::Initializing | Phase::Running | Phase::RetryWaiting
                )
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
        let now = executor::now();
        let mut admitted = false;
        self.status.send_modify(|status| {
            if status.cancellation_requested || status.phase == Phase::Settled {
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
        self.status.send_modify(|status| {
            if status.phase != Phase::Settled {
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
            let providers = std::mem::take(
                &mut *self
                    .providers
                    .lock()
                    .expect("instance providers are healthy"),
            );
            drop(providers);
            if let Some(notify) = &self.on_settled {
                notify();
            }
            let now = executor::now();
            let mut failure = None;
            self.status.send_modify(|status| {
                status.phase = Phase::Settled;
                status.cleanup_pending = false;
                failure = status.cleanup_failure.take();
                status.settled_at = Some(now);
            });
            drop(failure);
        });
    }

    pub(crate) fn interrupted(&self, failure: Arc<Error>) {
        let mut previous = None;
        self.status.send_modify(|status| {
            if status.phase != Phase::Settled {
                status.outcome.get_or_insert(Outcome::Unavailable);
                status.cleanup_pending = true;
                previous = status.cleanup_failure.replace(failure);
            }
        });
        drop(previous);
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.status.borrow().phase == Phase::Settled
    }

    pub(crate) fn track(&self, child: &Arc<Self>) {
        let mut children = self
            .children
            .lock()
            .expect("instance child lock is healthy");
        children.retain(|child| child.upgrade().is_some_and(|child| !child.is_settled()));
        children.push(Arc::downgrade(child));
        drop(children);
        if self.status().cancellation_requested {
            child.cancel();
        }
    }

    #[cfg(any(feature = "sidecar-host", test))]
    pub(crate) fn depend_on(
        self: &Arc<Self>,
        provider: &Arc<Self>,
        retirement: bool,
    ) -> crate::Result<()> {
        if Arc::ptr_eq(self, provider) {
            return Err(Error::InvalidInput {
                field: "physical provider",
                reason: "an instance cannot retain itself",
            });
        }
        let mut providers = self
            .providers
            .lock()
            .expect("instance providers are healthy");
        if providers
            .iter()
            .any(|held| Arc::ptr_eq(&held.provider, provider))
        {
            return Ok(());
        }
        let mut admitted = false;
        let mut stopping_control = false;
        provider.status.send_modify(|status| {
            if status.phase == Phase::Settled {
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
            return Err(Error::Closing {
                domain: "physical provider",
            });
        }
        providers.push(Dependency {
            provider: provider.clone(),
        });
        drop(providers);
        if !retirement {
            provider.track(self);
        }
        Ok(())
    }

    pub(crate) fn retain<R: Resource>(&self, resource: R, failure: Arc<Error>) {
        let mut retained = self
            .retained
            .lock()
            .expect("instance cleanup lock is healthy");
        assert!(
            retained.is_none(),
            "one instance has one physical cleanup owner"
        );
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
            let children: Vec<_> = self
                .children
                .lock()
                .expect("instance child lock is healthy")
                .iter()
                .filter_map(Weak::upgrade)
                .collect();
            let pending = children
                .into_iter()
                .map(|child| async move { child.settle().await });
            super::settle_all(pending).await
        })
    }

    pub(crate) fn settle(&self) -> Finalize<'_> {
        Box::pin(async move {
            let mut changed = self.settlement();
            loop {
                changed.borrow_and_update();
                self.settle_dependents().await?;
                let retained = self
                    .retained
                    .lock()
                    .expect("instance cleanup lock is healthy")
                    .clone();
                if let Some(retained) = retained {
                    if let Err(failure) = retained.settle().await {
                        let mut previous = None;
                        self.status.send_modify(|status| {
                            if status.phase != Phase::Settled {
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
                if status.phase == Phase::Settled {
                    return Ok(());
                }
                if let Some(failure) = status.cleanup_failure {
                    return Err(failure);
                }
                if changed.changed().await.is_err() {
                    return Err(Arc::new(Error::Closing {
                        domain: "instance settlement",
                    }));
                }
            }
        })
    }

    pub(crate) fn cancel(&self) {
        let mut interrupt = false;
        self.status.send_modify(|status| {
            interrupt = status.dependents == 0;
            status.cancellation_requested = true;
            if status.phase != Phase::Settled {
                status.cleanup_pending = true;
            }
            if !matches!(status.phase, Phase::Finalizing | Phase::Settled) {
                status.phase = Phase::Stopping;
            }
        });
        if interrupt {
            self.cancellation.send_replace(true);
        }
        let children: Vec<_> = self
            .children
            .lock()
            .expect("instance child lock is healthy")
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for child in children {
            child.cancel();
        }
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
            status.dependents = status
                .dependents
                .checked_sub(1)
                .expect("each provider hold releases once");
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
