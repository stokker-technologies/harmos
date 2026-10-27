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

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

use harmos_runtime::operations::*;
use harmos_runtime::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};
use std::sync::Arc;
use std::{sync::Mutex, time::Duration};

// The Request Carries The Shared Event Log, So It Stays In Process.
#[harmos_runtime::operation(codec = manual, resources = [harmos_runtime::operations::HostContext], id = "borrowed.request", response = u32, policy = OperationPolicy { timeout: Duration::from_secs(2), attempt_timeout: Duration::from_secs(1),
    retries: RetryPolicy::OnTransient { max_retries: 1,
        backoff: Backoff::Fixed { delay: Duration::from_millis(1), jitter: Jitter::None } } })]
#[derive(Clone)]
struct Request {
    value: u32,
    events: Arc<Mutex<Vec<&'static str>>>,
}
crate::operations::in_process!(Request);
impl harmos_runtime::MemorySize for Request {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}
impl Drop for Request {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("request dropped");
    }
}

impl harmos_runtime::Operation for Request {
    type Error = std::io::Error;
    async fn run(mut self, ctx: harmos_runtime::Context<'_, Self>) -> std::io::Result<harmos_runtime::Response<u32>> {
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        self.events.lock().unwrap().push("run");
        assert_eq!(self.value, 19);
        self.value += 1;
        if host.borrow().attempt() == 0 {
            return Err(std::io::Error::other("retry"));
        }
        Ok(self.value.into())
    }
    fn classify(_: &Self::Error) -> FailureKind {
        FailureKind::Transient
    }
}

#[tokio::test(start_paused = true)]
async fn retries_clone_the_original_request() {
    let runtime = harmos_runtime::Runtime::<crate::operations::probe::Editor>::builder(crate::operations::probe::Sheet::default(), ())
        .register(crate::operations::probe::Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(events.clone()).unwrap();
                dependencies
            })
            .operation::<Request>(),
            crate::operations::probe::Lanes::Open,
        ))
        .await
        .unwrap();
    let request = Request { value: 19, events: events.clone() };
    let call = runtime.operation(request).start().await.unwrap();
    assert_eq!(*call.join().await.unwrap(), 20);
    runtime.stop().await.unwrap();
    assert_eq!(*events.lock().unwrap(), ["run", "request dropped", "run", "request dropped", "request dropped"]);
}

#[tokio::test(start_paused = true)]
async fn transfer_blocks_retry() {
    use std::{
        num::NonZeroUsize,
        sync::atomic::{AtomicUsize, Ordering},
    };
    let runtime = harmos_runtime::Runtime::<crate::operations::probe::Editor>::builder(crate::operations::probe::Sheet::default(), ())
        .register(crate::operations::probe::Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    let runs = Arc::new(AtomicUsize::new(0));
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(runs.clone()).unwrap();
                dependencies
            })
            .operation::<Streaming>(),
            crate::operations::probe::Lanes::Open,
        ))
        .await
        .unwrap();
    let settings = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::new(2).unwrap(),
        bytes: NonZeroUsize::new(8).unwrap(),
    };
    for mode in [0, 1, 2] {
        runs.store(0, Ordering::SeqCst);
        let (send, input) = harmos_runtime::operations::stream::<u32>(settings).unwrap();
        let (output, receive) = harmos_runtime::operations::stream::<u32>(settings).unwrap();
        let instance = runtime.operations.submit_endpoints::<Streaming, _>(Streaming { mode }, (input, output)).unwrap();
        if mode == 2 {
            // The input item arrives while the failed attempt waits to retry.
            drop(
                instance
                    .subscribe()
                    .wait_for(|status| status.phase() == harmos_runtime::operations::OperationPhase::RetryWaiting)
                    .await
                    .unwrap(),
            );
            send.send_with(4, || Ok(9)).await.unwrap();
        }
        assert!(instance.join().await.is_err());
        assert_eq!(runs.load(Ordering::SeqCst), if mode == 0 { 2 } else { 1 });
        if mode == 1 {
            assert_eq!(*receive.recv().await.unwrap(), 7);
        }
    }
    runtime.stop().await.unwrap();
}

#[harmos_runtime::operation(harmos_path = harmos_runtime, resources = [std::sync::atomic::AtomicUsize],
    id = "borrowed.streaming",
    response = (),
    policy = Request::POLICY,
    input = u32,
    output = u32
)]
struct Streaming {
    mode: u32,
}

impl harmos_runtime::Operation for Streaming {
    type Error = std::io::Error;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> std::io::Result<harmos_runtime::Response<()>> {
        ctx.resource::<std::sync::atomic::AtomicUsize>().fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (_, output) = ctx.streams();
        if self.mode == 1 {
            output.send_with(4, || Ok(7)).await.map_err(std::io::Error::other)?;
        }
        Err(std::io::Error::other("transient stream failure"))
    }
    fn classify(_: &Self::Error) -> FailureKind {
        FailureKind::Transient
    }
}
