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

impl OperationDeclaration for Request {
    type Request = Self;
    type Response = u32;
    type Input = NoStream;
    type Output = NoStream;
    const ID: &'static str = "borrowed_request";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
    };
}
struct Read {
    events: Arc<Mutex<Vec<&'static str>>>,
    address: Option<usize>,
}
impl Handler for Read {
    type Request = <Request as harmos_runtime::OperationDeclaration>::Request;
    type Response = <Request as harmos_runtime::OperationDeclaration>::Response;
    type Error = std::io::Error;
    type StreamEndpoints = ();

    async fn initialize(_context: harmos_runtime::OperationContext) -> std::io::Result<Self> {
        let events = _context.resource::<Mutex<Vec<&'static str>>>().expect("fixture registers its dependency");
        let constructed: std::io::Result<Self> = async move {
            events.lock().unwrap().push("new");
            Ok(Self { events, address: None })
        }
        .await;
        let mut operation = constructed?;
        let started: std::io::Result<()> = async {
            let operation = &mut operation;
            operation.events.lock().unwrap().push("initialize");
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut Request, _: &mut Self::StreamEndpoints,
    ) -> std::io::Result<harmos_runtime::operations::Response<u32>> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            self.events.lock().unwrap().push("run");
            let address = std::ptr::from_ref(request) as usize;
            if let Some(original) = self.address {
                assert_eq!(original, address);
            }
            self.address = Some(address);
            request.value += 1;
            if request.value == 20 {
                return Err(std::io::Error::other("retry"));
            }
            Ok(request.value)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn stop(&mut self) -> std::io::Result<()> {
        self.events.lock().unwrap().push("stop");
        Ok(())
    }
    async fn cleanup(&mut self) -> std::io::Result<()> {
        let mut events = self.events.lock().unwrap();
        assert!(!events.contains(&"request dropped"));
        events.push("finalize");
        Ok(())
    }
    fn classify(_: &Self::Error) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }
}

#[tokio::test(start_paused = true)]
async fn request_outlives_cleanup() {
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
            .implementation::<Request, Read>(),
            crate::operations::probe::Lanes::Open,
        ))
        .await
        .unwrap();
    let request = Request { value: 19, events: events.clone() };
    let call = runtime.operation(request).start().await.unwrap();
    assert_eq!(*call.join().await.unwrap(), 21);
    runtime.stop().await.unwrap();
    assert_eq!(*events.lock().unwrap(), ["new", "initialize", "run", "run", "stop", "finalize", "request dropped"]);
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
            .implementation::<StreamingDeclarationDeclaration, Streaming>(),
            crate::operations::probe::Lanes::Open,
        ))
        .await
        .unwrap();
    let settings = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::new(2).unwrap(),
        bytes: NonZeroUsize::new(8).unwrap(),
    };
    for mode in [0, 1, 2, 3] {
        runs.store(0, Ordering::SeqCst);
        let (send, input) = harmos_runtime::operations::stream::<u32>(settings).unwrap();
        let (output, receive) = harmos_runtime::operations::stream::<u32>(settings).unwrap();
        let instance = runtime
            .operations
            .submit_endpoints::<StreamingDeclarationDeclaration, _>(StreamingDeclarationDeclaration { mode }, (input, output))
            .unwrap();
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

#[harmos_runtime::operation(harmos_path = harmos_runtime,
    id = "streaming",
    response = (),
    policy = Request::POLICY,
    input = u32,
    output = u32
)]
struct StreamingDeclarationDeclaration {
    mode: u32,
}

struct Streaming(Arc<std::sync::atomic::AtomicUsize>, bool);
impl harmos_runtime::Handler for Streaming {
    type Request = StreamingDeclarationDeclaration;
    type Response = ();
    type StreamEndpoints = (harmos_runtime::operations::StreamInput<u32>, harmos_runtime::operations::StreamOutput<u32>);
    type Error = std::io::Error;
    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self(context.resource::<std::sync::atomic::AtomicUsize>().map_err(std::io::Error::other)?, false))
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut StreamingDeclarationDeclaration,
        endpoints: &mut Self::StreamEndpoints,
    ) -> std::result::Result<harmos_runtime::Response<()>, std::io::Error> {
        let (_, output) = endpoints;
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Provider Transfer May Be Known Before Any Local Item Is Delivered.
        self.1 = request.mode == 3;
        if request.mode == 1 {
            output.send_with(4, || Ok(7)).await.map_err(std::io::Error::other)?;
        }
        Err(std::io::Error::other("transient stream failure"))
    }
    fn transferred(&self) -> bool {
        self.1
    }
    fn classify(_: &std::io::Error) -> harmos_runtime::operations::FailureKind {
        harmos_runtime::operations::FailureKind::Transient
    }
    #[cfg(any())]
    async fn stop(&mut self) -> std::result::Result<(), std::io::Error> {
        panic!("disabled lifecycle hook must use the trait default")
    }
    #[cfg_attr(all(), cfg_attr(all(), cfg(any())))]
    async fn cleanup(&mut self) -> std::result::Result<(), std::io::Error> {
        panic!("disabled lifecycle hook must use the trait default")
    }
}
