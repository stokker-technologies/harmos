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
use std::time::Duration;
// Third Party Imports
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio::time::error::Elapsed;
// Local Imports
use super::clock::Clock;
use super::entropy;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Executor {
    runtime: Runtime,
    clock: Clock,
    entropy: entropy::Context,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Executor {
    pub(crate) fn seeded(seed: u64) -> Self {
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().expect("the simulation current-thread executor builds");
        runtime.block_on(async { tokio::time::pause() });

        Self { runtime, clock: Clock::new(), entropy: entropy::Context::seeded(seed) }
    }

    pub(crate) fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(self.entropy.scope(future))
    }

    pub(crate) fn drive(&self) {
        self.block_on(async {
            for _ in 0..8 {
                tokio::task::yield_now().await;
            }
        });
    }

    pub(crate) fn advance_ms(&self, milliseconds: u64) {
        self.block_on(self.clock.advance_ms(milliseconds));
        self.drive();
    }

    pub(crate) fn elapsed_ms(&self) -> u64 {
        self.clock.elapsed_ms()
    }

    pub(crate) fn entropy_trace(&self) -> Vec<u64> {
        self.entropy.trace()
    }
}

/* === Public Functions === */
pub(crate) fn task<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match entropy::Context::current() {
        Some(context) => tokio::spawn(async move { context.scope(future).await }),
        None => tokio::spawn(future),
    }
}

pub(crate) fn entropy_or(fallback: u64) -> u64 {
    let context = entropy::Context::current();
    context.map_or(fallback, |context| context.draw())
}

pub(crate) fn now() -> tokio::time::Instant {
    tokio::time::Instant::now()
}

pub(crate) async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

pub(crate) async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, Elapsed>
where
    F: Future,
{
    tokio::time::timeout(duration, future).await
}

/* === Private Functions === */

/* === Tests === */
