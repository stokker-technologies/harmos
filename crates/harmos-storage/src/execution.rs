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
use std::task::{Context as TaskContext, Poll};
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
    #[tracing::instrument(name = "storage.execution.timeout", level = "debug", skip_all)]
    pub async fn timeout<F: Future>(self, duration: Duration, future: F) -> Result<F::Output, Elapsed> {
        tokio::select! { biased; result = future => Ok(result), () = self.sleep(duration) => Err(Elapsed) }
    }
}

impl Context {
    pub(crate) fn current() -> Self {
        Self { span: tracing::Span::current(), dispatch: tracing::dispatcher::get_default(Clone::clone) }
    }

    pub(crate) fn with_span(span: tracing::Span) -> Self {
        Self { span, dispatch: tracing::dispatcher::get_default(Clone::clone) }
    }

    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        Scoped { context: self.clone(), future: Some(Box::pin(future)) }.await
    }

    pub(crate) fn sync<T>(&self, action: impl FnOnce() -> T) -> T {
        tracing::dispatcher::with_default(&self.dispatch, || self.span.in_scope(action))
    }

    pub(crate) fn blocking<T>(&self, action: impl FnOnce() -> crate::Result<T>) -> crate::Result<T> {
        self.sync(|| {
            let span = tracing::debug_span!("storage.state.file.blocking", outcome = "cancelled");
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
