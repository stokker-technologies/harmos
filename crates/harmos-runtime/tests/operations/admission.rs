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
use std::num::NonZeroUsize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos_runtime::operations::{
    Context, Operation, OperationOutcome, OperationPhase, OperationPolicy, RestartPolicy, RetryPolicy, Settings,
};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The Hold Request Carries A Shared Notification, So It Stays In Process.
#[harmos_runtime::operation(
    response = (),
    id = "queue.hold",
    version = 1,
    policy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    },
)]
struct HoldContract {
    blocked: bool,
    started: Arc<Notify>,
}

struct Hold {
    counts: Arc<Counts>,
}

#[derive(Default)]
struct Counts {
    fail_cleanup: Arc<AtomicBool>,
    constructed: AtomicUsize,
    drops: AtomicUsize,
}

/* === Enums === */

#[harmos_runtime::catalog]
enum Operations {
    HoldContract(HoldContract),
}

/* === Traits === */

/* === Implementations === */

super::in_process!(HoldContract);

impl harmos_runtime::MemorySize for HoldContract {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl Operation for Hold {
    type Declaration = HoldContract;
    type Resources = Counts;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = std::io::Error;

    async fn initialize<'ctx>(counts: Arc<Counts>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        counts.constructed.fetch_add(1, Ordering::SeqCst);
        Ok(Self { counts })
    }

    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut HoldContract, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            request.started.notify_one();
            if request.blocked {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        if self.counts.fail_cleanup.load(Ordering::SeqCst) { Err(std::io::Error::other("retained queued resource")) } else { Ok(()) }
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.counts.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> Runtime<App> {
    Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(Settings {
            queued: NonZeroUsize::new(1).unwrap(),
            lifecycle_queued: NonZeroUsize::new(1).unwrap(),
            ..Settings::default()
        })
        .runtime()
        .await
        .unwrap()
}

fn request(blocked: bool) -> HoldContract {
    HoldContract { blocked, started: Arc::new(Notify::new()) }
}

/* === Tests === */

#[tokio::test]
async fn queue_refuses_construction() {
    let runtime = runtime().await;
    let counts = Arc::new(Counts::default());
    runtime.operations.register_operation::<Hold>("app", Lanes::Serial, counts.clone()).unwrap();
    let first_request = request(true);
    let started = first_request.started.clone();
    let first = runtime.operations.submit(first_request).unwrap();
    started.notified().await;
    let second = runtime.operations.submit(request(true)).unwrap();
    assert!(matches!(runtime.operations.submit(request(false)), Err(Error::ResourceExhausted { resource: "operation admission queue" })));

    runtime.stop().await.unwrap();
    drop((first, second));
}

#[tokio::test]
async fn queued_cancel_unbuilt() {
    let runtime = runtime().await;
    let counts = Arc::new(Counts::default());
    runtime.operations.register_operation::<Hold>("app", Lanes::Serial, counts.clone()).unwrap();
    let first_request = request(true);
    let started = first_request.started.clone();
    let first = runtime.operations.submit(first_request).unwrap();
    started.notified().await;
    let second_request = request(false);
    counts.fail_cleanup.store(true, Ordering::SeqCst);
    let mut second = runtime.operations.submit(second_request).unwrap();
    assert_eq!(second.status().phase(), OperationPhase::Queued);
    second.cancel();
    second.cancel();
    assert!(second.status().cancellation_requested());
    assert_eq!(second.status().phase(), OperationPhase::Stopping);
    // A cancelled queued instance never initialized, so it owns
    // nothing to clean up and releases its queue slot at once.
    assert!(matches!((&mut second).await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(second.status().phase(), OperationPhase::Settled);
    assert_eq!(second.status().outcome(), Some(OperationOutcome::Cancelled));
    assert!(second.status().started_at().is_none());
    second.settle().await.unwrap();
    assert!(runtime.operations.status(second.id()).is_none());
    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(counts.drops.load(Ordering::SeqCst), 0);
    counts.fail_cleanup.store(false, Ordering::SeqCst);
    let third = runtime.operations.submit(request(false)).unwrap();
    third.cancel();
    assert!(matches!(third.join().await, Err(Error::OperationRefused(Refusal::Cancelled))));
    first.cancel();
    assert!(matches!(first.join().await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn instances_leave_lookup() {
    let runtime = runtime().await;
    let counts = Arc::new(Counts::default());
    runtime.operations.register_operation::<Hold>("app", Lanes::Serial, counts.clone()).unwrap();
    for _ in 0..64 {
        let instance = runtime.operations.submit(request(false)).unwrap();
        let id = instance.id();
        instance.join().await.unwrap();
        assert!(runtime.operations.status(id).is_none());
    }
    assert_eq!(counts.drops.load(Ordering::SeqCst), 64);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn thread_submission_works() {
    let runtime = runtime().await;
    let counts = Arc::new(Counts::default());
    runtime.operations.register_operation::<Hold>("app", Lanes::Serial, counts.clone()).unwrap();
    let operations = runtime.operations.clone();
    let instance = std::thread::spawn(move || operations.submit(request(false))).join().unwrap().unwrap();
    instance.join().await.unwrap();
    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}
