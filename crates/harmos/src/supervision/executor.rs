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
use tokio::task::JoinHandle;
use tokio::time::error::Elapsed;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub(crate) fn task<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    #[cfg(feature = "sim")]
    {
        crate::sim::executor::task(future)
    }
    #[cfg(not(feature = "sim"))]
    {
        tokio::spawn(future)
    }
}

pub(crate) fn now() -> tokio::time::Instant {
    #[cfg(feature = "sim")]
    {
        crate::sim::executor::now()
    }
    #[cfg(not(feature = "sim"))]
    {
        tokio::time::Instant::now()
    }
}

pub(crate) async fn sleep(duration: Duration) {
    #[cfg(feature = "sim")]
    {
        crate::sim::executor::sleep(duration).await;
    }
    #[cfg(not(feature = "sim"))]
    {
        tokio::time::sleep(duration).await;
    }
}

pub(crate) async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, Elapsed>
where
    F: Future,
{
    #[cfg(feature = "sim")]
    {
        crate::sim::executor::timeout(duration, future).await
    }
    #[cfg(not(feature = "sim"))]
    {
        tokio::time::timeout(duration, future).await
    }
}

pub(crate) async fn yield_now() {
    tokio::task::yield_now().await;
}

/* === Private Functions === */

/* === Tests === */
