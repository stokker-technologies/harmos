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
use prost::Message;
use prost_types::Timestamp;

// Local Imports
use crate::{
    clock::{self, Sleep},
    supervision::instance::Instance,
};
use harmos_operations::clock::{Clock as OperationClock, ClockAccess};
use harmos_protocol::host::TimeSleepRequest;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Clock(pub(crate) clock::Clock);

struct RegisteredSleep {
    clock: clock::Clock,
    timer: Mutex<Option<Sleep>>,
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

    pub(crate) fn prepare_sleep(&self, operation: &str, payload: &[u8]) -> Option<ClockAccess> {
        if operation != "harmos.host.v1.Time/Sleep" {
            return None;
        }
        let request = TimeSleepRequest::decode(payload).ok()?;
        let duration = request.duration.unwrap_or_default();
        // Invalid Requests Still Use the Operation's Normal Validation.
        if duration.seconds < 0 || !(0..1_000_000_000).contains(&duration.nanos) || duration.seconds > 315_576_000_000 {
            return None;
        }
        let timer = self.clock.sleep(Duration::new(duration.seconds as u64, duration.nanos as u32));
        Some(ClockAccess(Arc::new(RegisteredSleep {
            clock: self.clock.clone(),
            timer: Mutex::new(Some(timer)),
        })))
    }
}

impl OperationClock for RegisteredSleep {
    fn now(&self) -> Timestamp {
        self.clock.date().into()
    }

    fn sleep(&self, _: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        let timer = self
            .timer
            .lock()
            .expect("scoped sleep registration lock is healthy")
            .take()
            .expect("Time.Sleep: scoped timer is registered before execution");
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
