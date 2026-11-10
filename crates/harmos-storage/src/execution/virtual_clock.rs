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
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, SystemTime};

// Third Party Imports
use tokio::sync::Notify;
use tokio::time::Instant;

// Local Imports
use super::{Clock, Execution, Reading, Sleep};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// The sole driver of a virtual host's time. Concurrent advances are serialized.
#[derive(Clone)]
pub struct Controller(Arc<Shared>);

struct Shared {
    state: Mutex<State>,
    changed: Notify,
    driver: tokio::sync::Mutex<()>,
}

struct State {
    origin: Reading,
    now: Instant,
    next_id: u64,
    timers: BTreeMap<(Instant, u64), Option<Waker>>,
    ready: BTreeSet<u64>,
    activities: BTreeMap<u64, (String, bool)>,
}

struct Timer {
    shared: Arc<Shared>,
    key: (Instant, u64),
}

/// Retain across an operation, unanswered tick or host I/O. Drop on settlement.
/// Each concurrent branch needs its own activity.
pub struct Activity {
    tracked: Option<(Arc<Shared>, u64)>,
}

struct Waiting<'a> {
    activity: &'a Activity,
}

struct Resume {
    shared: Arc<Shared>,
    id: u64,
    outer: Waker,
    awakened: AtomicBool,
}

/// Fixed cadence without a runtime timer or a background task.
pub struct Interval {
    clock: Execution,
    next: Instant,
    period: Duration,
}

#[derive(Debug, thiserror::Error)]
#[error("virtual time bound reached; pending timers: {pending_timers:?}")]
pub struct AdvanceError {
    pub pending_timers: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Controller {
    /// Advance to the next registered timer, waiting for work to become quiescent.
    pub async fn advance_next(&self) {
        let _driver = self.0.driver.lock().await;
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let deadline = self.0.state.lock().expect("virtual clock is healthy").timers.first_key_value().map(|(key, _)| key.0);
            if let Some(deadline) = deadline {
                self.drive(deadline, || false).await;
                return;
            }
            changed.await;
        }
    }

    pub fn read(&self) -> Reading {
        self.0.read()
    }

    /// Advance through each due timer, allowing its continuation to settle first.
    pub async fn advance_by(&self, duration: Duration) {
        let _driver = self.0.driver.lock().await;
        let target = self.read().instant + duration;
        self.drive(target, || false).await;
    }

    /// Stop at the first satisfied condition or return diagnostics at the bound.
    /// Active external work is awaited, never expired by virtual time; the test
    /// runner supplies the independent hang guard.
    pub async fn advance_until(&self, bound: Duration, condition: impl FnMut() -> bool) -> Result<(), AdvanceError> {
        let _driver = self.0.driver.lock().await;
        let target = self.read().instant + bound;
        if self.drive(target, condition).await {
            return Ok(());
        }
        let state = self.0.state.lock().expect("virtual clock is healthy");
        let pending_timers = state
            .timers
            .keys()
            .map(|(deadline, id)| format!("timer {id} at +{:?}", deadline.duration_since(state.origin.instant)))
            .collect();
        Err(AdvanceError { pending_timers })
    }

    async fn drive(&self, target: Instant, mut condition: impl FnMut() -> bool) -> bool {
        loop {
            // Register Before Inspecting State to Avoid Lost Activity Notifications
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if condition() {
                return true;
            }
            let wake = {
                let mut state = self.0.state.lock().expect("virtual clock is healthy");
                let active = state.activities.values().any(|(_, active)| *active);
                if active || !state.ready.is_empty() {
                    None
                } else if state.now >= target {
                    return false;
                } else {
                    let next = state.timers.first_key_value().map_or(target, |((deadline, _), _)| (*deadline).min(target));
                    state.now = next.max(state.now);
                    let mut wake = Vec::new();
                    while let Some((&key, _)) = state.timers.first_key_value() {
                        if key.0 > state.now {
                            break;
                        }
                        let waker = state.timers.remove(&key).unwrap();
                        state.ready.insert(key.1);
                        wake.extend(waker);
                    }
                    Some(wake)
                }
            };
            match wake {
                Some(wake) => {
                    // Restore Runnable Activity Before Considering Another Jump
                    for waker in wake {
                        waker.wake();
                    }
                    tokio::task::yield_now().await;
                }
                None => changed.await,
            }
        }
    }
}

impl Clock for Arc<Shared> {
    fn read(&self) -> Reading {
        let state = self.state.lock().expect("virtual clock is healthy");
        Reading {
            instant: state.now,
            date: state.origin.date + state.now.duration_since(state.origin.instant),
        }
    }

    fn sleep_until(&self, deadline: Instant) -> Sleep {
        let mut state = self.state.lock().expect("virtual clock is healthy");
        let id = state.next_id;
        state.next_id += 1;
        let key = (deadline, id);
        state.timers.insert(key, None);
        drop(state);
        self.changed.notify_waiters();
        Box::pin(Timer { shared: self.clone(), key })
    }

    fn activity(&self, name: String) -> Activity {
        let mut state = self.state.lock().expect("virtual clock is healthy");
        let id = state.next_id;
        state.next_id += 1;
        state.activities.insert(id, (name, true));
        Activity { tracked: Some((self.clone(), id)) }
    }
}

impl Future for Timer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let mut state = self.shared.state.lock().expect("virtual clock is healthy");
        if self.key.0 <= state.now {
            state.timers.remove(&self.key);
            state.ready.remove(&self.key.1);
            drop(state);
            self.shared.changed.notify_waiters();
            return Poll::Ready(());
        }
        *state.timers.get_mut(&self.key).expect("pending timer remains registered") = Some(context.waker().clone());
        Poll::Pending
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().expect("virtual clock is healthy");
        state.timers.remove(&self.key);
        state.ready.remove(&self.key.1);
        drop(state);
        self.shared.changed.notify_waiters();
    }
}

impl Activity {
    pub(super) fn untracked() -> Self {
        Self { tracked: None }
    }

    /// Park only while this branch's sole outstanding work is a clock wait.
    /// Do not wrap a timeout around active work: a deadline is not quiescence.
    pub async fn wait<F: Future>(&mut self, future: F) -> F::Output {
        let waiting = Waiting { activity: self };
        let mut future = Box::pin(future);
        std::future::poll_fn(|context| {
            let Some((shared, id)) = &waiting.activity.tracked else {
                return future.as_mut().poll(context);
            };
            let resume = Arc::new(Resume {
                shared: shared.clone(),
                id: *id,
                outer: context.waker().clone(),
                awakened: AtomicBool::new(false),
            });
            let waker = Waker::from(resume.clone());
            shared.state.lock().expect("virtual clock is healthy").activities.get_mut(id).unwrap().1 = true;
            let result = future.as_mut().poll(&mut Context::from_waker(&waker));
            if result.is_pending() {
                // Register the Wait Before Parking; Serialize Parking with Its Wake
                let mut state = shared.state.lock().expect("virtual clock is healthy");
                state.activities.get_mut(id).unwrap().1 = resume.awakened.load(Ordering::SeqCst);
            }
            shared.changed.notify_waiters();
            result
        })
        .await
    }
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        if let Some((shared, id)) = &self.activity.tracked {
            if let Some((_, active)) = shared.state.lock().expect("virtual clock is healthy").activities.get_mut(id) {
                *active = true;
            }
            shared.changed.notify_waiters();
        }
    }
}

impl Wake for Resume {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        {
            let mut state = self.shared.state.lock().expect("virtual clock is healthy");
            self.awakened.store(true, Ordering::SeqCst);
            if let Some((_, active)) = state.activities.get_mut(&self.id) {
                *active = true;
            }
        }
        self.shared.changed.notify_waiters();
        self.outer.wake_by_ref();
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        if let Some((shared, id)) = &self.tracked {
            shared.state.lock().expect("virtual clock is healthy").activities.remove(id);
            shared.changed.notify_waiters();
        }
    }
}

impl Interval {
    pub(super) fn new(clock: Execution, period: Duration) -> Self {
        assert!(!period.is_zero(), "clock interval must be nonzero");
        let next = clock.now() + period;
        Self { clock, next, period }
    }

    pub async fn tick(&mut self) -> Reading {
        let deadline = self.next;
        self.clock.sleep_until(deadline).await;
        self.next += self.period;
        let reading = self.clock.read();
        Reading {
            instant: deadline,
            date: reading.date - reading.instant.duration_since(deadline),
        }
    }
}

/* === Public Functions === */

pub(super) fn create(date: SystemTime) -> (Execution, Controller) {
    let origin = Reading { instant: Instant::now(), date };
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            origin,
            now: origin.instant,
            next_id: 0,
            timers: BTreeMap::new(),
            ready: BTreeSet::new(),
            activities: BTreeMap::new(),
        }),
        changed: Notify::new(),
        driver: tokio::sync::Mutex::new(()),
    });
    (Execution::with_clock(shared.clone()), Controller(shared))
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timers_wake_in_deadline_order_and_cancel_on_drop() {
        let (clock, controller) = create(SystemTime::UNIX_EPOCH);
        let order = Mutex::new(Vec::new());
        let late = clock.sleep(Duration::from_secs(3));
        let early = clock.sleep(Duration::from_secs(1));
        let cancelled = clock.sleep(Duration::from_secs(2));
        drop(cancelled);
        let work = async {
            tokio::join!(
                async {
                    late.await;
                    order.lock().unwrap().push(3);
                },
                async {
                    early.await;
                    order.lock().unwrap().push(1);
                }
            );
        };
        tokio::join!(work, controller.advance_by(Duration::from_secs(3)));
        assert_eq!(*order.lock().unwrap(), [1, 3]);
        assert!(controller.0.state.lock().unwrap().timers.is_empty());
        assert_eq!(controller.read().date, SystemTime::UNIX_EPOCH + Duration::from_secs(3));
    }

    struct RecordWake {
        order: Arc<Mutex<Vec<u64>>>,
        id: u64,
    }

    impl Wake for RecordWake {
        fn wake(self: Arc<Self>) {
            self.order.lock().unwrap().push(self.id);
        }
    }

    #[tokio::test]
    async fn equal_deadlines_are_registered_and_woken_in_order() {
        let (clock, controller) = create(SystemTime::UNIX_EPOCH);
        let mut first = clock.sleep(Duration::from_secs(1));
        let mut second = clock.sleep(Duration::from_secs(1));
        let order = Arc::new(Mutex::new(Vec::new()));
        for (id, timer) in [(1, &mut first), (2, &mut second)] {
            let waker = Waker::from(Arc::new(RecordWake { order: order.clone(), id }));
            assert!(timer.as_mut().poll(&mut Context::from_waker(&waker)).is_pending());
        }
        let mut advance = Box::pin(controller.advance_by(Duration::from_secs(1)));
        assert!(std::future::poll_fn(|cx| Poll::Ready(advance.as_mut().poll(cx).is_pending())).await);
        assert_eq!(*order.lock().unwrap(), [1, 2]);
        first.await;
        second.await;
        advance.await;
        assert!(controller.0.state.lock().unwrap().ready.is_empty());
    }

    #[tokio::test]
    async fn activity_blocks_deadlines_and_resumes_before_the_next_jump() {
        let (clock, controller) = create(SystemTime::UNIX_EPOCH);
        let origin = clock.now();
        let mut call = clock.activity("operation");
        let external = clock.activity("disk work");
        let mut advance = Box::pin(controller.advance_by(Duration::from_secs(10)));
        let pending = std::future::poll_fn(|cx| Poll::Ready(advance.as_mut().poll(cx).is_pending())).await;
        assert!(pending);
        assert_eq!(clock.now(), origin);
        drop(external);
        let done = AtomicBool::new(false);
        let work = async {
            call.wait(clock.sleep(Duration::from_secs(2))).await;
            assert_eq!(clock.now() - origin, Duration::from_secs(2));
            // An Unanswered Tick or External Reply Remains Active Across a Yield
            tokio::task::yield_now().await;
            assert_eq!(clock.now() - origin, Duration::from_secs(2));
            done.store(true, Ordering::SeqCst);
            drop(call);
        };
        tokio::join!(work, advance);
        assert!(done.load(Ordering::SeqCst));
        assert_eq!(clock.now() - origin, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn bound_reports_pending_timers_and_timeout_uses_the_queue() {
        let (clock, controller) = create(SystemTime::UNIX_EPOCH);
        let sleep = clock.sleep(Duration::from_secs(5));
        let error = controller.advance_until(Duration::from_secs(2), || false).await.unwrap_err();
        assert_eq!(error.pending_timers.len(), 1);
        assert!(error.to_string().contains("timer 0 at +5s"));
        drop(sleep);
        let (result, ()) = tokio::join!(
            clock.timeout(Duration::from_secs(1), std::future::pending::<()>()),
            controller.advance_by(Duration::from_secs(1))
        );
        assert_eq!(result, Err(super::super::Elapsed));
    }

    #[tokio::test]
    async fn wake_during_registration_keeps_activity_runnable() {
        let (clock, controller) = create(SystemTime::UNIX_EPOCH);
        let mut activity = clock.activity("registering wait");
        let mut first = true;
        let future = std::future::poll_fn(|context| {
            if first {
                first = false;
                context.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        });
        let mut wait = Box::pin(activity.wait(future));
        assert!(std::future::poll_fn(|cx| Poll::Ready(wait.as_mut().poll(cx).is_pending())).await);
        assert!(controller.0.state.lock().unwrap().activities.values().next().unwrap().1);
        wait.await;
        assert!(controller.0.state.lock().unwrap().activities.values().next().unwrap().1);
    }

    #[tokio::test]
    async fn cancelling_a_parked_wait_restores_activity_and_removes_its_timer() {
        let (clock, controller) = create(SystemTime::UNIX_EPOCH);
        let mut activity = clock.activity("call");
        let mut wait = Box::pin(activity.wait(clock.sleep(Duration::from_secs(5))));
        assert!(std::future::poll_fn(|cx| Poll::Ready(wait.as_mut().poll(cx).is_pending())).await);
        assert!(!controller.0.state.lock().unwrap().activities.values().next().unwrap().1);
        drop(wait);
        let state = controller.0.state.lock().unwrap();
        assert!(state.activities.values().next().unwrap().1);
        assert!(state.timers.is_empty());
    }
}
