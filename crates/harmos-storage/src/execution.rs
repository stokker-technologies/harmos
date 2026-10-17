/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::Instant;

/* === Type Aliases === */
pub type Sleep = Pin<Box<dyn Future<Output = ()> + Send>>;

/* === Structs === */
#[derive(Clone, Copy)]
pub struct Execution {
    now: fn() -> Instant,
    sleep: fn(Duration) -> Sleep,
}

/* === Implementations === */
impl Execution {
    pub const fn new(now: fn() -> Instant, sleep: fn(Duration) -> Sleep) -> Self { Self { now, sleep } }
    pub fn now(self) -> Instant { (self.now)() }
    pub fn sleep(self, duration: Duration) -> Sleep { (self.sleep)(duration) }
    pub async fn timeout<F: Future>(self, duration: Duration, future: F) -> Result<F::Output, ()> {
        tokio::select! { biased; result = future => Ok(result), () = self.sleep(duration) => Err(()) }
    }
}
impl Default for Execution {
    fn default() -> Self {
        Self::new(Instant::now, |duration| Box::pin(tokio::time::sleep(duration)))
    }
}
