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
use harmos_operations::clock::WindDown;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Clock(pub(crate) clock::Clock);

/// One Call's Unit Clock; Admission Registers the Call's Declared Wait Before Middleware Runs.
pub(crate) struct Registered {
    clock: clock::Clock,
    wind_down: WindDown,
    windows: Option<Arc<Windows>>,
    timer: Mutex<Option<(Duration, Instant, Sleep)>>,
}

pub(crate) struct Unit {
    pub(crate) clock: clock::Clock,
    pub(crate) wind_down: WindDown,
    pending: Mutex<Option<Vec<Weak<Instance>>>>,
    pub(crate) windows: Arc<Windows>,
}

/// The Next Ungranted Sample of Each Interval Demand in One Unit, and the Latest Requested Cut.
#[derive(Default)]
pub(crate) struct Windows {
    due: watch::Sender<BTreeMap<u64, Instant>>,
    cut: watch::Sender<Option<Instant>>,
    next: AtomicU64,
}

/// One Demand's Place in Its Unit's Window Schedule; Dropping It Leaves the Schedule.
pub(crate) struct Schedule {
    windows: Arc<Windows>,
    id: u64,
}

/// Marks a Window's Call Tree, Whose Waits Its Own Window Cannot Settle Before.
pub(crate) struct WindowCall;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Unit {
    pub(crate) fn new(clock: clock::Clock) -> Self {
        Self {
            clock,
            wind_down: WindDown::default(),
            pending: Mutex::new(Some(Vec::new())),
            windows: Arc::default(),
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

impl Windows {
    pub(crate) fn schedule(self: &Arc<Self>, due: Instant) -> Schedule {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.due.send_modify(|windows| {
            windows.insert(id, due);
        });
        Schedule { windows: self.clone(), id }
    }

    /// Ask Every Demand to Grant Through `at`, Then Resolve Once Each Has Committed Every Sample Due by Then.
    async fn committed_through(&self, at: Instant) {
        self.cut.send_modify(|cut| *cut = Some(cut.map_or(at, |cut| cut.max(at))));
        let mut due = self.due.subscribe();
        let _ = due.wait_for(|windows| windows.values().all(|next| *next > at)).await;
    }
}

impl Schedule {
    /// Record the Demand's Next Ungranted Sample After Its Previous Window Committed.
    pub(crate) fn due(&self, next: Instant) {
        self.windows.due.send_modify(|windows| {
            windows.insert(self.id, next);
        });
    }

    /// Resolve When a Scoped Wait Asks for the Windows Due at or After `next`.
    pub(crate) async fn cut(&self, next: Instant) {
        let mut cut = self.windows.cut.subscribe();
        let _ = cut.wait_for(|cut| cut.is_some_and(|cut| cut >= next)).await;
    }
}

impl Drop for Schedule {
    fn drop(&mut self) {
        self.windows.due.send_modify(|windows| {
            windows.remove(&self.id);
        });
    }
}

impl Registered {
    /// A Call Outside Its Unit's Windows Orders Its Sleeps After the Windows They Span.
    pub(crate) fn new(clock: clock::Clock, windows: Option<Arc<Windows>>, wind_down: WindDown) -> Self {
        Self {
            clock,
            windows,
            wind_down,
            timer: Mutex::new(None),
        }
    }
}

impl harmos_operations::clock::Clock for Registered {
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
        // A Sleep Ending at `deadline` Settles After Every Unit Sample Due by Then Has Committed.
        let windows = self.windows.clone();
        Box::pin(clock::waiting(self.wind_down.wait(async move {
            timer.await;
            if let Some(windows) = windows {
                windows.committed_through(deadline).await;
            }
        })))
    }
}

impl harmos_operations::clock::Clock for Clock {
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
