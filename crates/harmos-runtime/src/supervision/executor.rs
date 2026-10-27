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
use tracing::Instrument;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct SpanContext {
    pub(crate) span: tracing::Span,
    pub(crate) dispatcher: tracing::Dispatch,
    queued: Option<tracing::Span>,
}

pub(crate) struct Contextual<F: std::future::Future> {
    future: Option<std::pin::Pin<Box<F>>>,
    dispatcher: tracing::Dispatch,
}

/* === Enums === */

/* === Traits === */

pub(crate) trait InContext: std::future::Future + Sized {
    fn in_context(self, dispatcher: tracing::Dispatch) -> Contextual<Self> {
        Contextual { future: Some(Box::pin(self)), dispatcher }
    }

    fn in_current_context(self) -> Contextual<Self> {
        self.in_context(tracing::dispatcher::get_default(Clone::clone))
    }
}

/* === Implementations === */

impl SpanContext {
    pub(crate) fn new(span: tracing::Span, queued: tracing::Span) -> Self {
        Self {
            span,
            dispatcher: tracing::dispatcher::get_default(Clone::clone),
            queued: Some(queued),
        }
    }

    pub(crate) fn predecessor(&self) -> Option<tracing::Id> {
        self.queued.as_ref().and_then(tracing::Span::id)
    }

    pub(crate) fn dequeued(&mut self) {
        tracing::dispatcher::with_default(&self.dispatcher, || drop(self.queued.take()));
    }
}

impl Drop for SpanContext {
    fn drop(&mut self) {
        let span = std::mem::replace(&mut self.span, tracing::Span::none());
        let queued = self.queued.take();
        tracing::dispatcher::with_default(&self.dispatcher, || drop((span, queued)));
    }
}

impl<F: std::future::Future> InContext for F {}

impl<F: std::future::Future> std::future::Future for Contextual<F> {
    type Output = F::Output;

    fn poll(self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let owner = self.get_mut();
        let _dispatcher = tracing::dispatcher::set_default(&owner.dispatcher);
        owner.future.as_mut().expect("context future remains until drop").as_mut().poll(context)
    }
}

impl<F: std::future::Future> Drop for Contextual<F> {
    fn drop(&mut self) {
        tracing::dispatcher::with_default(&self.dispatcher, || drop(self.future.take()));
    }
}

/* === Public Functions === */

pub(crate) fn task<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let future = future.instrument(tracing::Span::current()).in_current_context();
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

/* === Private Functions === */

/* === Tests === */
