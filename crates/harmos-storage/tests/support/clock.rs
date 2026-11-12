/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Advance Policy Time Only When The Story Requests It

/* === Imports === */

use harmos_storage::execution::{Clock, Execution, Reading, Sleep};
use std::time::{Duration, SystemTime};

/* === Structs === */

#[derive(Clone)]
pub(crate) struct Controlled(tokio::sync::watch::Sender<Reading>);

/* === Implementations === */

impl Controlled {
    pub(crate) fn new() -> Self {
        let reading = Reading {
            instant: tokio::time::Instant::now(),
            date: SystemTime::UNIX_EPOCH,
        };
        Self(tokio::sync::watch::channel(reading).0)
    }

    pub(crate) fn source(&self) -> Execution {
        Execution::with_clock(self.clone())
    }

    pub(crate) fn advance(&self, duration: Duration) {
        self.0.send_modify(|reading| {
            reading.instant += duration;
            reading.date += duration;
        });
    }
}

impl Clock for Controlled {
    fn read(&self) -> Reading {
        *self.0.borrow()
    }

    fn sleep_until(&self, deadline: tokio::time::Instant) -> Sleep {
        let mut changed = self.0.subscribe();
        Box::pin(async move {
            let _ = changed.wait_for(|reading| reading.instant >= deadline).await;
        })
    }
}
