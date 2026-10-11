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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Wake, Waker};
// Third Party Imports

// Local Imports
use super::{Notice, Supervised};
use crate::services::Resident;
use crate::{Application, Fault, Service};
use harmos_sidecar::Attach;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Attempt = Pin<Box<dyn Future<Output = Result<(), Fault<String>>> + Send>>;
type Ending = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/* === Structs === */
pub(super) struct Physical<A: Application, S, H> {
    original: Arc<Mutex<Original<A, S, H>>>,
}

pub(super) struct Polling<A: Application, S, H> {
    original: Weak<Mutex<Original<A, S, H>>>,
}

struct Original<A: Application, S, H> {
    signals: Arc<Signals>,
    running_busy: bool,
    ending_busy: bool,
    cancelled: bool,
    running: Option<Attempt>,
    ending: Option<Ending>,
    settled: bool,
    ending_failure: Option<String>,
    recovery: Arc<Mutex<Option<Supervised<A, S, H>>>>,
    outcome: Option<Result<(), Fault<String>>>,
}

#[derive(Default)]
struct Signals {
    revision: AtomicU64,
    waiting: Mutex<Vec<Waker>>,
}

struct Owned<A: Application, S, H> {
    resident: Option<Supervised<A, S, H>>,
    recovery: Arc<Mutex<Option<Supervised<A, S, H>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Signals {
    fn register(&self, waker: &Waker) {
        let mut waiting = self
            .waiting
            .lock()
            .expect("physical wake registry is healthy");
        if !waiting.iter().any(|current| current.will_wake(waker)) {
            waiting.push(waker.clone());
        }
    }
    fn notify(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
        let waiting = std::mem::take(
            &mut *self
                .waiting
                .lock()
                .expect("physical wake registry is healthy"),
        );
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
        Self {
            original: self.original.clone(),
        }
    }
}

impl<A: Application, S, H> Owned<A, S, H> {
    // Borrow Through The Guard So The Future Captures Its Recovery Owner.
    fn resident(&mut self) -> &mut Supervised<A, S, H> {
        self.resident
            .as_mut()
            .expect("original physical resident retained")
    }
}

impl<A: Application, S, H> Drop for Owned<A, S, H> {
    fn drop(&mut self) {
        let Some(resident) = self.resident.take() else {
            return;
        };
        let mut recovery = self
            .recovery
            .lock()
            .expect("physical owner recovery poisoned");
        assert!(
            recovery.is_none(),
            "the original physical resident has one owner"
        );
        *recovery = Some(resident);
    }
}

impl<A, S, H> Physical<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    pub(super) fn new(resident: Supervised<A, S, H>, context: Service<A>) -> Self {
        let recovery = Arc::new(Mutex::new(None));
        let mut owner = Owned {
            resident: Some(resident),
            recovery: Arc::clone(&recovery),
        };
        let running = Box::pin(async move { owner.resident().attempt(context).await });
        Self {
            original: Arc::new(Mutex::new(Original {
                signals: Arc::default(),
                running_busy: false,
                ending_busy: false,
                cancelled: false,
                running: Some(running),
                ending: None,
                settled: false,
                ending_failure: None,
                recovery,
                outcome: None,
            })),
        }
    }

    pub(super) fn polling(&self) -> Polling<A, S, H> {
        Polling {
            original: Arc::downgrade(&self.original),
        }
    }

    fn signals(&self, context: &Context<'_>) -> Arc<Signals> {
        let signals = self
            .original
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .signals
            .clone();
        signals.register(context.waker());
        signals
    }

    pub(super) fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Fault<String>>> {
        let signals = self.signals(context);
        let mut running = {
            let mut original = self
                .original
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(outcome) = &original.outcome {
                return Poll::Ready(outcome.clone());
            }
            if original.running_busy {
                return Poll::Pending;
            }
            let Some(running) = original.running.take() else {
                return Poll::Ready(Err(Fault::Application(
                    "native physical attempt was interrupted".into(),
                )));
            };
            original.running_busy = true;
            running
        };
        let revision = signals.revision.load(Ordering::Acquire);
        let waker = Waker::from(signals.clone());
        let mut polling = Context::from_waker(&waker);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            running.as_mut().poll(&mut polling)
        }));
        match result {
            Ok(Poll::Pending) => {
                let mut original = self
                    .original
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let cancelled = original.cancelled;
                if cancelled {
                    drop(original);
                    drop(running);
                    self.original
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .running_busy = false;
                } else {
                    original.running = Some(running);
                    original.running_busy = false;
                    drop(original);
                }
                if cancelled || signals.revision.load(Ordering::Acquire) != revision {
                    signals.notify();
                }
                Poll::Pending
            }
            Ok(Poll::Ready(outcome)) => {
                drop(running);
                let mut original = self
                    .original
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                original.running_busy = false;
                original.outcome = Some(outcome.clone());
                drop(original);
                signals.notify();
                Poll::Ready(outcome)
            }
            Err(panic) => {
                drop(running);
                let mut original = self
                    .original
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                original.running_busy = false;
                original.outcome = Some(Err(Fault::Application(
                    "native physical attempt panicked".into(),
                )));
                drop(original);
                signals.notify();
                std::panic::resume_unwind(panic)
            }
        }
    }

    pub(super) fn is_running(&self) -> bool {
        let original = self
            .original
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        original.running.is_some() || original.running_busy
    }

    pub(super) fn cancel(&mut self) {
        let (running, signals) = {
            let mut original = self
                .original
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            original.cancelled = true;
            let running = original.running.take();
            if running.is_some() {
                original.running_busy = true;
            }
            (running, original.signals.clone())
        };
        if let Some(running) = running {
            drop(running);
            self.original
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .running_busy = false;
        }
        signals.notify();
    }

    pub(super) fn retry_ending(&mut self) {
        self.original
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .ending_failure
            .take();
    }

    pub(super) fn poll_ending(&mut self, context: &mut Context<'_>) -> Poll<Result<(), String>> {
        let signals = self.signals(context);
        let (ending, recovery) = {
            let mut original = self
                .original
                .lock()
                .unwrap_or_else(|error| error.into_inner());
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
            (original.ending.take(), Arc::clone(&original.recovery))
        };
        let mut ending = ending.unwrap_or_else(|| {
            let resident = recovery
                .lock()
                .expect("physical owner recovery poisoned")
                .take()
                .expect("physical future returned its original resident");
            let mut owner = Owned {
                resident: Some(resident),
                recovery,
            };
            Box::pin(async move {
                owner.resident().stop().await?;
                owner.resident().finalize().await
            })
        });
        let revision = signals.revision.load(Ordering::Acquire);
        let waker = Waker::from(signals.clone());
        let mut polling = Context::from_waker(&waker);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ending.as_mut().poll(&mut polling)
        }));
        match result {
            Ok(Poll::Pending) => {
                let mut original = self
                    .original
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                original.ending_busy = false;
                original.ending = Some(ending);
                drop(original);
                if signals.revision.load(Ordering::Acquire) != revision {
                    signals.notify();
                }
                Poll::Pending
            }
            Ok(Poll::Ready(outcome)) => {
                drop(ending);
                let mut original = self
                    .original
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                original.ending_busy = false;
                match &outcome {
                    Ok(()) => original.settled = true,
                    Err(failure) => original.ending_failure = Some(failure.clone()),
                }
                drop(original);
                signals.notify();
                Poll::Ready(outcome)
            }
            Err(panic) => {
                drop(ending);
                let mut original = self
                    .original
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                original.ending_busy = false;
                original.ending_failure = Some("native physical cleanup panicked".into());
                drop(original);
                signals.notify();
                std::panic::resume_unwind(panic)
            }
        }
    }
}

impl<A, S, H> Polling<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    pub(super) fn poll(&self, context: &mut Context<'_>) -> Poll<Result<(), Fault<String>>> {
        let Some(original) = self.original.upgrade() else {
            return Poll::Pending;
        };
        let mut physical = Physical { original };
        if physical.is_running() {
            physical.poll(context)
        } else {
            Poll::Pending
        }
    }
    pub(super) fn retry_ending(&self) {
        if let Some(original) = self.original.upgrade() {
            Physical { original }.retry_ending();
        }
    }

    pub(super) fn poll_ending(&self, context: &mut Context<'_>) -> Poll<Result<(), String>> {
        let Some(original) = self.original.upgrade() else {
            return Poll::Pending;
        };
        let mut physical = Physical { original };
        if physical.is_running()
            || physical
                .original
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .settled
        {
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
