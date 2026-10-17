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

use harmos::operations::*;
use harmos::operations::{Backoff, Jitter, OperationPolicy, RestartPolicy, RetryPolicy};
use std::sync::Arc;
use std::{cell::Cell, sync::Mutex, time::Duration};

struct Request {
    value: Cell<u32>,
    events: Arc<Mutex<Vec<&'static str>>>,
}
impl Drop for Request {
    fn drop(&mut self) {
        self.events.lock().unwrap().push("request dropped");
    }
}
#[harmos::lanes]
enum LifecycleLanes {
    #[default]
    Owned,
}

struct Declaration;
impl OperationDeclaration for Declaration {
    type Request = Request;
    type Response = u32;
    const ID: &'static str = "borrowed.request";
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
        restart: RestartPolicy::Never,
    };
}
struct Read {
    events: Arc<Mutex<Vec<&'static str>>>,
    address: Option<usize>,
}
impl Operation for Read {
    type Declaration = Declaration;
    type Resources = Mutex<Vec<&'static str>>;
    type Error = std::io::Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(
        events: Arc<Self::Resources>,
        _context: Self::Context<'ctx>,
    ) -> std::io::Result<Self> {
        let constructed: std::io::Result<Self> = async move {
            events.lock().unwrap().push("new");
            Ok(Self {
                events,
                address: None,
            })
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
    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Request,
        _: &mut Self::Endpoints,
    ) -> std::io::Result<harmos::operations::Response<u32>> {
        let result: ::core::result::Result<u32, Self::Error> = async move {
            self.events.lock().unwrap().push("run");
            let address = std::ptr::from_ref(request) as usize;
            if let Some(original) = self.address {
                assert_eq!(original, address);
            }
            self.address = Some(address);
            request.value.set(request.value.get() + 1);
            if request.value.get() == 20 {
                return Err(std::io::Error::other("retry"));
            }
            Ok(request.value.get())
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
    fn classify(_: &Self::Error) -> harmos::operations::FailureKind {
        harmos::operations::FailureKind::Transient
    }
}

#[tokio::test]
async fn owned_request_lifecycle() {
    let runtime =
        harmos::Runtime::<crate::probe::Editor>::builder(crate::probe::Sheet::default(), ())
            .register(LifecycleLanes::catalog())
            .runtime()
            .await
            .unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    runtime
        .operations
        .register_operation::<Read>("owner", LifecycleLanes::Owned, events.clone())
        .unwrap();
    let request = Request {
        value: Cell::new(19),
        events: events.clone(),
    };
    let call = runtime
        .operations
        .submit_as::<Declaration>(request)
        .unwrap();
    assert_eq!(*call.join().await.unwrap(), 21);
    runtime.stop().await.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "new",
            "initialize",
            "run",
            "run",
            "stop",
            "finalize",
            "request dropped"
        ]
    );
}

#[tokio::test(start_paused = true)]
async fn transfer_fences_retries() {
    use std::{
        num::NonZeroUsize,
        sync::atomic::{AtomicUsize, Ordering},
    };
    let runtime =
        harmos::Runtime::<crate::probe::Editor>::builder(crate::probe::Sheet::default(), ())
            .register(LifecycleLanes::catalog())
            .runtime()
            .await
            .unwrap();
    let runs = Arc::new(AtomicUsize::new(0));
    runtime
        .operations
        .register_operation::<Streaming>("owner", LifecycleLanes::Owned, runs.clone())
        .unwrap();
    let settings = harmos::operations::StreamSettings {
        items: NonZeroUsize::new(2).unwrap(),
        bytes: NonZeroUsize::new(8).unwrap(),
    };
    for mode in [0, 1, 2, 3] {
        runs.store(0, Ordering::SeqCst);
        let (send, input) = harmos::operations::stream::<u32>(settings).unwrap();
        let (output, mut receive) = harmos::operations::stream::<u32>(settings).unwrap();
        let instance = runtime
            .operations
            .submit_endpoints::<StreamingDeclaration, _>(mode, (input, output))
            .unwrap();
        if mode == 2 {
            for _ in 0..1000 {
                if instance.status().phase() == harmos::operations::OperationPhase::RetryWaiting {
                    break;
                }
                tokio::task::yield_now().await;
            }
            assert_eq!(
                instance.status().phase(),
                harmos::operations::OperationPhase::RetryWaiting
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

struct Streaming(Arc<std::sync::atomic::AtomicUsize>, bool);
#[harmos::operation(harmos_path = harmos, policy = Declaration::POLICY, input = u32, output = u32)]
impl Streaming {
    async fn initialize(resources: Arc<std::sync::atomic::AtomicUsize>, _: Context<'_>) -> Self {
        Self(resources, false)
    }
    async fn run(
        &mut self,
        _: Context<'_>,
        mode: &u32,
        _: &mut harmos::operations::StreamInput<u32>,
        output: &mut harmos::operations::StreamOutput<u32>,
    ) -> std::result::Result<(), std::io::Error> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Provider Transfer May Be Known Before Any Local Item Is Delivered.
        self.1 = *mode == 3;
        if *mode == 1 {
            output
                .send_with(4, || Ok(7))
                .await
                .map_err(std::io::Error::other)?;
        }
        Err(std::io::Error::other("transient stream failure"))
    }
    fn transferred(&self) -> bool {
        self.1
    }
    fn classify(_: &std::io::Error) -> harmos::operations::FailureKind {
        harmos::operations::FailureKind::Transient
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
