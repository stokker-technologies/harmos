/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::{future::Future, io, sync::mpsc, thread};

use tokio::{runtime::Builder, sync::oneshot, task::JoinHandle};
use tracing::Instrument;

struct Contextual<F: std::future::Future> {
    future: Option<std::pin::Pin<Box<F>>>,
    dispatcher: tracing::Dispatch,
}

trait InContext: std::future::Future + Sized {
    fn in_context(self, dispatcher: tracing::Dispatch) -> Contextual<Self> {
        Contextual { future: Some(Box::pin(self)), dispatcher }
    }

    fn in_current_context(self) -> Contextual<Self> {
        self.in_context(tracing::dispatcher::get_default(Clone::clone))
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

pub fn spawn(name: String, work: impl Future<Output = ()> + Send + 'static) -> io::Result<JoinHandle<()>> {
    let work = work.instrument(tracing::Span::current()).in_current_context();
    let (started, ready) = mpsc::sync_channel(1);
    thread::Builder::new().name(name).spawn(move || {
        let runtime = match Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = started.send(Err(error));
                return;
            }
        };
        let (finished, completion) = oneshot::channel::<()>();
        let task = runtime.spawn(async move {
            // Dropping this guard also wakes the runtime owner after an abort
            // or panic. It must live until the author's future has dropped.
            let _finished = finished;
            work.await;
        });
        if started.send(Ok(task)).is_err() {
            return;
        }

        let _ = runtime.block_on(completion);
    })?;

    ready.recv().map_err(io::Error::other)?
}
