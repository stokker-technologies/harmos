/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Supply Runtime Clock Resources to Host Operations.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

// Third Party Imports
use prost_types::Timestamp;
use tokio::{sync::watch, time::Instant};

// Local Imports
use crate::{
    clock::{self, Sleep},
    supervision::instance::Instance,
};
use harmos_operations::clock::Clock as OperationClock;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Clock(pub(crate) clock::Clock);

/// One Call's Unit Clock; Admission Registers the Call's Declared Wait Before Middleware Runs.
pub(crate) struct Registered {
    clock: clock::Clock,
    ticks: Option<Arc<Ticks>>,
    timer: Mutex<Option<(Duration, Instant, Sleep)>>,
}

pub(crate) struct Unit {
    pub(crate) clock: clock::Clock,
    pending: Mutex<Option<Vec<Weak<Instance>>>>,
    pub(crate) ticks: Arc<Ticks>,
}

/// The Next Unanswered Tick of Each Interval Demand in One Unit.
#[derive(Default)]
pub(crate) struct Ticks {
    due: watch::Sender<BTreeMap<u64, Instant>>,
    next: AtomicU64,
}

/// One Demand's Place in Its Unit's Tick Schedule; Dropping It Leaves the Schedule.
pub(crate) struct Schedule {
    ticks: Arc<Ticks>,
    id: u64,
}

/// Marks a Sample Tick's Call Tree, Whose Sleeps Its Own Tick Cannot Wait For.
pub(crate) struct TickCall;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Unit {
    pub(crate) fn new(clock: clock::Clock) -> Self {
        Self {
            clock,
            pending: Mutex::new(Some(Vec::new())),
            ticks: Arc::default(),
        }
    }

    pub(crate) fn track(&self, instance: &Arc<Instance>) -> bool {
        let mut pending = self.pending.lock().expect("scoped work lock is healthy");
        let Some(pending) = pending.as_mut() else {
            return false;
        };
        pending.retain(|instance| instance.strong_count() != 0);
        pending.push(Arc::downgrade(instance));
        true
    }

    pub(crate) fn stop(&self) -> Vec<Arc<Instance>> {
        let pending = self.pending.lock().expect("scoped work lock is healthy").take().unwrap_or_default();
        let pending = pending.into_iter().filter_map(|instance| instance.upgrade()).collect::<Vec<_>>();
        for instance in &pending {
            instance.request_cancel();
        }
        pending
    }
}

impl Ticks {
    pub(crate) fn schedule(self: &Arc<Self>, due: Instant) -> Schedule {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.due.send_modify(|ticks| {
            ticks.insert(id, due);
        });
        Schedule { ticks: self.clone(), id }
    }

    /// Resolve Once Every Tick Due at or Before `at` Has Been Answered and Its Sample Committed.
    async fn answered_through(&self, at: Instant) {
        let mut due = self.due.subscribe();
        let _ = due.wait_for(|ticks| ticks.values().all(|next| *next > at)).await;
    }
}

impl Schedule {
    /// Record the Demand's Next Unanswered Tick After Its Previous Sample Committed.
    pub(crate) fn due(&self, next: Instant) {
        self.ticks.due.send_modify(|ticks| {
            ticks.insert(self.id, next);
        });
    }
}

impl Drop for Schedule {
    fn drop(&mut self) {
        self.ticks.due.send_modify(|ticks| {
            ticks.remove(&self.id);
        });
    }
}

impl Registered {
    /// A Call Outside Its Unit's Sample Ticks Orders Its Sleeps After the Ticks They Span.
    pub(crate) fn new(clock: clock::Clock, ticks: Option<Arc<Ticks>>) -> Self {
        Self { clock, ticks, timer: Mutex::new(None) }
    }
}

impl OperationClock for Registered {
    fn now(&self) -> Timestamp {
        self.clock.date().into()
    }

    fn register(&self, duration: Duration) {
        let deadline = self.clock.now() + duration;
        let timer = self.clock.sleep_until(deadline);
        *self.timer.lock().expect("scoped sleep registration lock is healthy") = Some((duration, deadline, timer));
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        // A Retried Attempt Has Consumed the Admitted Timer and Registers Its Own.
        let registered = self.timer.lock().expect("scoped sleep registration lock is healthy").take();
        let (deadline, timer) = match registered {
            Some((admitted, deadline, timer)) if admitted == duration => (deadline, timer),
            _ => {
                let deadline = self.clock.now() + duration;
                (deadline, self.clock.sleep_until(deadline))
            }
        };
        // A Sleep Ending at `deadline` Settles After Every Unit Tick Due by Then Has Committed.
        let ticks = self.ticks.clone();
        Box::pin(clock::waiting(async move {
            timer.await;
            if let Some(ticks) = ticks {
                ticks.answered_through(deadline).await;
            }
        }))
    }
}

impl OperationClock for Clock {
    fn now(&self) -> Timestamp {
        self.0.date().into()
    }
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(clock::waiting(self.0.sleep(duration)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
