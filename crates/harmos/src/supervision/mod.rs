/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
pub(crate) mod cleanup;
#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub(crate) mod dependencies;
pub(crate) mod executor;
pub(crate) mod instance;

#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
mod listeners;
mod task;

/* === Re-Exports === */
#[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
pub(crate) use listeners::{Delivery as ListenerDelivery, Listener};
pub(crate) use task::{Task, settle_all};

/* === Imports === */
// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
use tokio::task::{JoinError, JoinHandle};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Backoff;

struct Attempt<T>(JoinHandle<T>);

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    Panicked,
    StopTimedOut,
}

/* === Traits === */

/* === Implementations === */
impl Backoff {
    pub(crate) fn duration(attempt: usize, base: Duration, cap: Duration) -> Duration {
        let shift = u32::try_from(attempt.min(16)).expect("the capped attempt fits u32");
        let exponential = base
            .saturating_mul(1_u32.checked_shl(shift).unwrap_or(u32::MAX))
            .min(cap);
        let jitter_range = exponential.as_millis().saturating_div(4).saturating_add(1);
        let jitter = ((attempt as u128).wrapping_mul(17).wrapping_add(11)) % jitter_range;

        exponential.saturating_add(Duration::from_millis(jitter as u64))
    }
}

impl<T> Future for Attempt<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().0).poll(context)
    }
}

impl<T> Drop for Attempt<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/* === Public Functions === */
pub(crate) async fn attempt<F>(
    future: F,
    mut stopping: watch::Receiver<bool>,
    stop_timeout: Duration,
) -> Result<F::Output, Failure>
where
    F: Future + Send,
    F::Output: Send,
{
    let mut future = std::pin::pin!(future);
    let mut running = std::pin::pin!(std::future::poll_fn(|context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(poll) => poll.map(Ok),
            Err(_) => Poll::Ready(Err(Failure::Panicked)),
        }
    }));
    let completed = interruptible(running.as_mut(), &mut stopping).await;

    if let Some(answer) = completed {
        return answer;
    }

    match executor::timeout(stop_timeout, running).await {
        Ok(answer) => answer,
        Err(_) => Err(Failure::StopTimedOut),
    }
}

pub(crate) async fn interruptible<F: Future>(
    future: F,
    stopping: &mut watch::Receiver<bool>,
) -> Option<F::Output> {
    let mut future = std::pin::pin!(future);
    let stopped = async {
        loop {
            if *stopping.borrow_and_update() {
                return;
            }
            if stopping.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    };
    let mut stopped = std::pin::pin!(stopped);

    std::future::poll_fn(|context| {
        if stopped.as_mut().poll(context).is_ready() {
            return Poll::Ready(None);
        }
        future.as_mut().poll(context).map(Some)
    })
    .await
}

pub(crate) async fn joined<F>(future: F) -> Result<F::Output, JoinError>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    Attempt(executor::task(future)).await
}

/* === Private Functions === */

/* === Tests === */
