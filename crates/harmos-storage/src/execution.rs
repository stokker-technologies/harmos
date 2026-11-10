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
use std::sync::{Arc, RwLock};
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, SystemTime};

// Third Party Imports
use tokio::time::Instant;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Sleep = Pin<Box<dyn Future<Output = ()> + Send>>;

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("clock deadline elapsed")]
pub struct Elapsed;

#[derive(Clone)]
pub struct Execution {
    source: Arc<RwLock<Arc<dyn Clock>>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub instant: Instant,
    pub date: SystemTime,
}

struct RealClock {
    origin: Reading,
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
    sleep: Arc<dyn Fn(Duration) -> Sleep + Send + Sync>,
}

#[derive(Clone)]
pub(crate) struct Context {
    pub(crate) span: tracing::Span,
    dispatch: tracing::Dispatch,
}

struct Scoped<F> {
    context: Context,
    future: Option<Pin<Box<F>>>,
}

/* === Enums === */

/* === Traits === */

/// A host time source. Sleeps register on this source, including absolute deadlines.
pub trait Clock: Send + Sync + 'static {
    fn read(&self) -> Reading;
    fn sleep_until(&self, deadline: Instant) -> Sleep;
}

/* === Implementations === */

impl Execution {
    pub fn new(now: impl Fn() -> Instant + Send + Sync + 'static, sleep: impl Fn(Duration) -> Sleep + Send + Sync + 'static) -> Self {
        Self::at(SystemTime::now(), now, sleep)
    }

    pub fn real_at(date: SystemTime) -> Self {
        Self::at(date, Instant::now, |duration| Box::pin(tokio::time::sleep(duration)))
    }

    pub fn at(
        date: SystemTime, now: impl Fn() -> Instant + Send + Sync + 'static, sleep: impl Fn(Duration) -> Sleep + Send + Sync + 'static,
    ) -> Self {
        let origin = Reading { instant: now(), date };
        Self::with_clock(RealClock {
            origin,
            now: Arc::new(now),
            sleep: Arc::new(sleep),
        })
    }

    pub fn with_clock(clock: impl Clock) -> Self {
        Self {
            source: Arc::new(RwLock::new(Arc::new(clock))),
        }
    }

    pub fn same_source(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.source, &other.source) {
            return true;
        }
        let source = self.source.read().expect("clock source is healthy");
        let other = other.source.read().expect("clock source is healthy");
        Arc::ptr_eq(&source, &other)
    }

    pub(crate) fn adopt(&self, other: &Self) {
        let source = other.source.read().expect("clock source is healthy").clone();
        *self.source.write().expect("clock source is healthy") = source;
    }

    pub fn read(&self) -> Reading {
        let source = self.source.read().expect("clock source is healthy").clone();
        source.read()
    }

    pub fn now(&self) -> Instant {
        self.read().instant
    }

    pub fn date(&self) -> SystemTime {
        self.read().date
    }

    pub fn sleep(&self, duration: Duration) -> Sleep {
        self.sleep_until(self.now() + duration)
    }

    pub fn sleep_until(&self, deadline: Instant) -> Sleep {
        let source = self.source.read().expect("clock source is healthy").clone();
        source.sleep_until(deadline)
    }

    #[tracing::instrument(name = "storage.execution.timeout", level = "debug", skip_all)]
    pub async fn timeout<F: Future>(&self, duration: Duration, future: F) -> Result<F::Output, Elapsed> {
        Box::pin(self.timeout_at(self.now() + duration, future)).await
    }

    pub async fn timeout_at<F: Future>(&self, deadline: Instant, future: F) -> Result<F::Output, Elapsed> {
        let sleep = self.sleep_until(deadline);
        Box::pin(async move {
            tokio::select! { biased; result = future => Ok(result), () = sleep => Err(Elapsed) }
        })
        .await
    }
}

impl Clock for RealClock {
    fn read(&self) -> Reading {
        let instant = (self.now)();
        let date = self.origin.date + instant.saturating_duration_since(self.origin.instant);
        Reading { instant, date }
    }

    fn sleep_until(&self, deadline: Instant) -> Sleep {
        (self.sleep)(deadline.saturating_duration_since((self.now)()))
    }
}

impl Context {
    pub(crate) fn current() -> Self {
        Self {
            span: tracing::Span::current(),
            dispatch: tracing::dispatcher::get_default(Clone::clone),
        }
    }

    pub(crate) fn with_span(span: tracing::Span) -> Self {
        Self {
            span,
            dispatch: tracing::dispatcher::get_default(Clone::clone),
        }
    }

    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        Scoped {
            context: self.clone(),
            future: Some(Box::pin(future)),
        }
        .await
    }

    pub(crate) async fn result<T>(&self, span: tracing::Span, future: impl Future<Output = crate::Result<T>>) -> crate::Result<T> {
        // Keep both handles inside the captured dispatcher through completion and drop.
        let outcome_span = span.clone();
        let future = async move {
            let result = future.await;
            outcome_span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
            result
        };
        self.scope(tracing::Instrument::instrument(future, span)).await
    }

    pub(crate) fn sync<T>(&self, action: impl FnOnce() -> T) -> T {
        tracing::dispatcher::with_default(&self.dispatch, || self.span.in_scope(action))
    }

    pub(crate) fn blocking<T>(&self, action: impl FnOnce() -> crate::Result<T>) -> crate::Result<T> {
        self.sync(|| {
            let span = tracing::debug_span!("storage.backend.blocking", outcome = "cancelled");
            span.in_scope(|| {
                let result = action();
                span.record("outcome", if result.is_ok() { "completed" } else { "failed" });
                result
            })
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        let span = std::mem::replace(&mut self.span, tracing::Span::none());
        tracing::dispatcher::with_default(&self.dispatch, || drop(span));
    }
}

impl<F: Future> Future for Scoped<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, task: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let context = &this.context;
        let future = this.future.as_mut().expect("scoped future remains owned while polling");
        context.sync(|| future.as_mut().poll(task))
    }
}

impl<F> Drop for Scoped<F> {
    fn drop(&mut self) {
        let future = self.future.take();
        self.context.sync(|| drop(future));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(epoch: SystemTime) -> Execution {
        Execution::at(epoch, Instant::now, |duration| Box::pin(tokio::time::sleep(duration)))
    }

    #[tokio::test(start_paused = true)]
    async fn date_and_monotonic_readings_share_elapsed_time() {
        let epoch = SystemTime::UNIX_EPOCH + Duration::from_secs(123);
        let clock = clock(epoch);
        let first = clock.read();
        tokio::time::advance(Duration::from_secs(3600)).await;
        let second = clock.read();
        assert_eq!(second.instant - first.instant, Duration::from_secs(3600));
        assert_eq!(second.date.duration_since(first.date).unwrap(), second.instant - first.instant);
        assert_eq!(first.date, epoch);
    }

    #[tokio::test(start_paused = true)]
    async fn absolute_sleep_and_timeout_share_the_source() {
        let clock = clock(SystemTime::UNIX_EPOCH);
        let deadline = clock.now() + Duration::from_secs(60);
        let mut sleep = clock.sleep_until(deadline);
        assert!(std::future::poll_fn(|cx| Poll::Ready(sleep.as_mut().poll(cx).is_pending())).await);
        tokio::time::advance(Duration::from_secs(59)).await;
        assert!(std::future::poll_fn(|cx| Poll::Ready(sleep.as_mut().poll(cx).is_pending())).await);
        tokio::time::advance(Duration::from_secs(1)).await;
        sleep.await;
        let expired = clock.timeout_at(deadline, std::future::pending::<()>()).await;
        assert_eq!(expired, Err(Elapsed));
        assert_eq!(clock.timeout(Duration::ZERO, std::future::ready(7)).await, Ok(7));
    }

    #[tokio::test(start_paused = true)]
    async fn adoption_updates_preexisting_handles_to_the_host_source() {
        let storage = clock(SystemTime::UNIX_EPOCH);
        let retained = storage.clone();
        let host = clock(SystemTime::UNIX_EPOCH + Duration::from_secs(123));
        storage.adopt(&host);
        tokio::time::advance(Duration::from_secs(5)).await;
        assert!(retained.same_source(&host));
        assert_eq!(retained.date(), host.date());
        assert_eq!(retained.now(), host.now());
    }

    #[tokio::test(start_paused = true)]
    async fn cloned_clock_retains_epoch_and_independent_hosts_keep_their_dates() {
        let first = clock(SystemTime::UNIX_EPOCH);
        let retained = first.clone();
        let second = clock(SystemTime::UNIX_EPOCH + Duration::from_secs(100));
        drop(first);
        tokio::time::advance(Duration::from_secs(5)).await;
        assert_eq!(retained.date(), SystemTime::UNIX_EPOCH + Duration::from_secs(5));
        assert_eq!(second.date(), SystemTime::UNIX_EPOCH + Duration::from_secs(105));
    }
}
