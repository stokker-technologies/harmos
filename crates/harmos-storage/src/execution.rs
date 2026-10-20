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
use std::time::Duration;

// Third Party Imports
use tokio::time::Instant;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Sleep = Pin<Box<dyn Future<Output = ()> + Send>>;

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Elapsed;

#[derive(Clone, Copy)]
pub struct Execution {
    now: fn() -> Instant,
    sleep: fn(Duration) -> Sleep,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Execution {
    pub const fn new(now: fn() -> Instant, sleep: fn(Duration) -> Sleep) -> Self {
        Self { now, sleep }
    }
    pub fn now(self) -> Instant {
        (self.now)()
    }
    pub fn sleep(self, duration: Duration) -> Sleep {
        (self.sleep)(duration)
    }
    pub async fn timeout<F: Future>(self, duration: Duration, future: F) -> Result<F::Output, Elapsed> {
        tokio::select! { biased; result = future => Ok(result), () = self.sleep(duration) => Err(Elapsed) }
    }
}

impl Default for Execution {
    fn default() -> Self {
        Self::new(Instant::now, |duration| Box::pin(tokio::time::sleep(duration)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
