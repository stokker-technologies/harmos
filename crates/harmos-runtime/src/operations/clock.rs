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
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

// Third Party Imports
use prost_types::Timestamp;

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
    timer: Mutex<Option<(Duration, Sleep)>>,
}

pub(crate) struct Unit {
    pub(crate) clock: clock::Clock,
    pending: Mutex<Option<Vec<Weak<Instance>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Unit {
    pub(crate) fn new(clock: clock::Clock) -> Self {
        Self {
            clock,
            pending: Mutex::new(Some(Vec::new())),
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

impl Registered {
    pub(crate) fn new(clock: clock::Clock) -> Self {
        Self { clock, timer: Mutex::new(None) }
    }
}

impl OperationClock for Registered {
    fn now(&self) -> Timestamp {
        self.clock.date().into()
    }

    fn register(&self, duration: Duration) {
        let timer = self.clock.sleep(duration);
        *self.timer.lock().expect("scoped sleep registration lock is healthy") = Some((duration, timer));
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        // A Retried Attempt Has Consumed the Admitted Timer and Registers Its Own.
        let registered = self.timer.lock().expect("scoped sleep registration lock is healthy").take();
        let timer = match registered {
            Some((admitted, timer)) if admitted == duration => timer,
            _ => self.clock.sleep(duration),
        };
        Box::pin(clock::waiting(timer))
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
