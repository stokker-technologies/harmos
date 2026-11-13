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
pub(crate) struct Controlled {
    reading: tokio::sync::watch::Sender<Reading>,
    scheduled: tokio::sync::watch::Sender<Vec<tokio::time::Instant>>,
}

/* === Implementations === */

impl Controlled {
    pub(crate) fn new() -> Self {
        let reading = Reading {
            instant: tokio::time::Instant::now(),
            date: SystemTime::UNIX_EPOCH,
        };
        Self {
            reading: tokio::sync::watch::channel(reading).0,
            scheduled: tokio::sync::watch::channel(Vec::new()).0,
        }
    }

    pub(crate) fn source(&self) -> Execution {
        Execution::with_clock(self.clone())
    }

    pub(crate) async fn scheduled(&self, duration: Duration) {
        let deadline = self.read().instant + duration;
        let mut scheduled = self.scheduled.subscribe();
        let _ = scheduled.wait_for(|deadlines| deadlines.contains(&deadline)).await;
    }

    pub(crate) fn advance(&self, duration: Duration) {
        self.reading.send_modify(|reading| {
            reading.instant += duration;
            reading.date += duration;
        });
    }
}

impl Clock for Controlled {
    fn read(&self) -> Reading {
        *self.reading.borrow()
    }

    fn sleep_until(&self, deadline: tokio::time::Instant) -> Sleep {
        self.scheduled.send_modify(|deadlines| deadlines.push(deadline));
        let mut changed = self.reading.subscribe();
        Box::pin(async move {
            let _ = changed.wait_for(|reading| reading.instant >= deadline).await;
        })
    }
}
