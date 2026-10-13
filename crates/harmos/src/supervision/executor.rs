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
    #[cfg(feature = "simulation")]
    {
        crate::simulation::executor::task(future)
    }

    #[cfg(not(feature = "simulation"))]
    {
        tokio::spawn(future)
    }
}

pub(crate) fn now() -> tokio::time::Instant {
    #[cfg(feature = "simulation")]
    {
        crate::simulation::executor::now()
    }

    #[cfg(not(feature = "simulation"))]
    {
        tokio::time::Instant::now()
    }
}

pub(crate) async fn sleep(duration: Duration) {
    #[cfg(feature = "simulation")]
    {
        crate::simulation::executor::sleep(duration).await;
    }

    #[cfg(not(feature = "simulation"))]
    {
        tokio::time::sleep(duration).await;
    }
}

pub(crate) async fn timeout<F>(duration: Duration, future: F) -> Result<F::Output, Elapsed>
where
    F: Future,
{
    #[cfg(feature = "simulation")]
    {
        crate::simulation::executor::timeout(duration, future).await
    }

    #[cfg(not(feature = "simulation"))]
    {
        tokio::time::timeout(duration, future).await
    }
}

pub(crate) async fn yield_now() {
    tokio::task::yield_now().await;
}

/* === Private Functions === */

/* === Tests === */
