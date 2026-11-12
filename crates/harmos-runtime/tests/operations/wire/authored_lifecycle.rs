/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Tests === */

use harmos_runtime::operations::*;
use harmos_runtime::operations::{Backoff, Jitter, OperationPolicy, RetryPolicy};
use std::sync::Arc;
use std::{sync::Mutex, time::Duration};

// The Request Carries The Shared Event Log, So It Stays In Process.
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u32)]
#[derive(Clone)]
struct Request {
    value: u32,
    events: Arc<Mutex<Vec<&'static str>>>,
}

static REQUEST_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<Request>(
    "borrowed.request",
    OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
    },
);

const REQUEST_OPERATION: harmos_runtime::operations::OperationSpec<Request> =
    harmos_runtime::operations::OperationSpec::<Request>::new(&REQUEST_DESCRIPTION);
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

async fn request_handler(
    mut request: Request, ctx: harmos_runtime::operations::FunctionContext<Request>,
) -> std::io::Result<harmos_runtime::Response<u32>> {
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    request.events.lock().unwrap().push("run");
    assert_eq!(request.value, 19);
    request.value += 1;
    if host.borrow().attempt() == 0 {
        return Err(std::io::Error::other("retry"));
    }
    Ok(request.value.into())
}

fn request_classification(_: &std::io::Error) -> FailureKind {
    FailureKind::Transient
}

fn request_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        REQUEST_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        request_handler,
        request_classification,
    )
}

#[tokio::test(start_paused = true)]
async fn retries_clone_the_original_request() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let runtime =
        harmos_runtime::Runtime::<crate::operations::probe::Editor>::launch_with(crate::operations::probe::Sheet::default(), (), |setup| {
            setup.operation(request_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
        })
        .await
        .unwrap();
    let request = Request { value: 19, events: events.clone() };
    let call = runtime.operation(request).start().await.unwrap();
    assert_eq!(*call.join().await.unwrap(), 20);
    crate::support::app::shutdown(&runtime).await.unwrap();
    assert_eq!(*events.lock().unwrap(), ["run", "request dropped", "run", "request dropped", "request dropped"]);
}

#[tokio::test(start_paused = true)]
async fn transfer_blocks_retry() {
    use std::{
        num::NonZeroUsize,
        sync::atomic::{AtomicUsize, Ordering},
    };
    let runs = Arc::new(AtomicUsize::new(0));
    let runtime =
        harmos_runtime::Runtime::<crate::operations::probe::Editor>::launch_with(crate::operations::probe::Sheet::default(), (), |setup| {
            setup
                .resource::<AtomicUsize>(runs.clone())
                .operation(streaming_operation().expect("representable operation"), harmos_runtime::operations::Lane::default());
        })
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
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[derive(harmos_runtime::Message, Debug, Default)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = (), input = u32, output = u32)]
#[derive(Clone)]
struct Streaming {
    mode: u32,
}

static STREAMING_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Streaming>("borrowed.streaming", REQUEST_OPERATION.entry().policy);

const STREAMING_OPERATION: harmos_runtime::operations::OperationSpec<Streaming> =
    harmos_runtime::operations::OperationSpec::<Streaming>::new(&STREAMING_DESCRIPTION);

async fn streaming_handler(
    request: Streaming, ctx: harmos_runtime::operations::FunctionContext<Streaming>,
) -> std::io::Result<harmos_runtime::Response<()>> {
    ctx.resource::<std::sync::atomic::AtomicUsize>()
        .expect("declared resource is registered")
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let (_, output) = ctx.streams();
    if request.mode == 1 {
        output.send_with(4, || Ok(7)).await.map_err(std::io::Error::other)?;
    }
    Err(std::io::Error::other("transient stream failure"))
}

fn streaming_classification(_: &std::io::Error) -> FailureKind {
    FailureKind::Transient
}

fn streaming_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        STREAMING_OPERATION,
        const { &[harmos_runtime::Resource::of::<std::sync::atomic::AtomicUsize>()] },
        streaming_handler,
        streaming_classification,
    )
}
