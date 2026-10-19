/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Assert That A Future Stays Parked Without Waiting On The Clock

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

// Third Party Imports

// Local Imports

/* === Constants === */

// Scheduler turns granted to the tasks a parked future waits on.
const SETTLE: usize = 64;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Poll `future` while every other ready task runs, asserting it stays pending.
pub(crate) async fn parked<F: Future + ?Sized>(mut future: Pin<&mut F>) {
    for _ in 0..SETTLE {
        assert!(poll_once(future.as_mut()).await, "the future stays parked");
        tokio::task::yield_now().await;
    }
}

/// Assert `future` stays pending across `span` of the paused clock.
pub(crate) async fn outlasts<F: Future + ?Sized>(mut future: Pin<&mut F>, span: Duration) {
    parked(future.as_mut()).await;
    tokio::time::advance(span).await;
    parked(future).await;
}

/* === Private Functions === */

async fn poll_once<F: Future + ?Sized>(mut future: Pin<&mut F>) -> bool {
    std::future::poll_fn(|context| Poll::Ready(future.as_mut().poll(context).is_pending())).await
}

/* === Tests === */
