/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Clock Declarations and Narrow Clock Access.

/* === Modules === */

mod get;
mod sleep;

/* === Re-Exports === */

pub use get::Get;
pub use sleep::Sleep;

/* === Imports === */

// Standard Library Imports
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

// Third Party Imports
use tokio::sync::watch;

// Local Imports

/* === Constants === */

const POLICY: crate::OperationPolicy = crate::OperationPolicy {
    timeout: harmos_operations::operations::UNBOUNDED,
    attempt_timeout: harmos_operations::operations::UNBOUNDED,
    cancellation_timeout: Duration::ZERO,
    retries: crate::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct ClockAccess(pub Arc<dyn Clock>);

/// A Sticky Scope Signal That Releases Explicit Waits Without Cancelling Work.
#[derive(Clone, Default)]
pub struct WindDown(watch::Sender<bool>);

/* === Enums === */

/* === Traits === */

pub trait Clock: Send + Sync {
    fn now(&self) -> prost_types::Timestamp;
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Register the Wait an Admitted Call Will Perform, Before Middleware Observes the Call.
    fn register(&self, _duration: Duration) {}
}

/* === Implementations === */

impl WindDown {
    pub fn request(&self) {
        self.0.send_replace(true);
    }

    pub fn is_requested(&self) -> bool {
        *self.0.borrow()
    }

    pub async fn requested(&self) {
        let mut requested = self.0.subscribe();
        let _ = requested.wait_for(|requested| *requested).await;
    }

    /// Release a Wait Normally; Dropping the Pending Wait Removes Its Registration.
    pub async fn wait(&self, future: impl Future<Output = ()>) {
        tokio::select! {
            biased;
            () = self.requested() => {},
            () = future => {},
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::WindDown;
    use std::future::{pending, poll_fn};
    use std::task::Poll;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn wind_down_releases_all_pending_and_later_waits_normally() {
        let scope = WindDown::default();
        let (first, ready_first) = oneshot::channel();
        let (second, ready_second) = oneshot::channel();
        let mut first = Some(first);
        let mut second = Some(second);
        let waiting_first = poll_fn(move |_| {
            if let Some(first) = first.take() {
                first.send(()).unwrap();
            }
            Poll::Pending::<()>
        });
        let waiting_second = poll_fn(move |_| {
            if let Some(second) = second.take() {
                second.send(()).unwrap();
            }
            Poll::Pending::<()>
        });
        let request = async {
            ready_first.await.unwrap();
            ready_second.await.unwrap();
            assert!(!scope.is_requested());
            scope.request();
        };
        tokio::join!(scope.wait(waiting_first), scope.wait(waiting_second), request);
        assert!(scope.is_requested());
        scope.wait(pending()).await;
        scope.request();
        scope.wait(pending()).await;
    }
}
