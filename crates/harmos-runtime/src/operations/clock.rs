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
use std::sync::{Arc, Mutex, Weak};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Clock(pub(crate) crate::clock::Clock);

pub(crate) struct Unit {
    pub(crate) clock: crate::clock::Clock,
    pending: Mutex<Option<Vec<Weak<crate::supervision::instance::Instance>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Unit {
    pub(crate) fn new(clock: crate::clock::Clock) -> Self {
        Self {
            clock,
            pending: Mutex::new(Some(Vec::new())),
        }
    }

    pub(crate) fn track(&self, instance: &Arc<crate::supervision::instance::Instance>) -> bool {
        let mut pending = self.pending.lock().expect("scoped work lock is healthy");
        let Some(pending) = pending.as_mut() else {
            return false;
        };
        pending.retain(|instance| instance.strong_count() != 0);
        pending.push(Arc::downgrade(instance));
        true
    }

    pub(crate) fn stop(&self) -> Vec<Arc<crate::supervision::instance::Instance>> {
        let pending = self.pending.lock().expect("scoped work lock is healthy").take().unwrap_or_default();
        let pending = pending.into_iter().filter_map(|instance| instance.upgrade()).collect::<Vec<_>>();
        for instance in &pending {
            instance.request_cancel();
        }
        pending
    }
}

impl harmos_operations::clock::Clock for Clock {
    fn now(&self) -> prost_types::Timestamp {
        self.0.date().into()
    }
    fn sleep(&self, duration: std::time::Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(crate::clock::waiting(self.0.sleep(duration)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
