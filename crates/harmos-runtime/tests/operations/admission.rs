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
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::app::App;
use harmos_runtime::operations::{OperationOutcome, OperationPhase, RetryPolicy, Settings};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The Hold Request Carries A Shared Notification, So It Stays In Process.
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = ())]
#[derive(Clone)]
struct HoldContractDeclaration {
    blocked: bool,
    started: Arc<Notify>,
}

static HOLD_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<HoldContractDeclaration>(
        "queue.hold",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const HOLD_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<HoldContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<HoldContractDeclaration>::new(&HOLD_CONTRACT_DECLARATION_DESCRIPTION);

struct AttemptGuard {
    counts: Arc<Counts>,
}

#[derive(Default)]
struct Counts {
    constructed: AtomicUsize,
    drops: AtomicUsize,
}

/* === Enums === */

fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(hold_contract_declaration_operation().expect("representable operation"));
    batch
}

/* === Traits === */

/* === Implementations === */

super::in_process!(HoldContractDeclaration);

impl harmos_runtime::MemorySize for HoldContractDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

async fn hold_contract_declaration_handler(
    request: HoldContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<HoldContractDeclaration>,
) -> Result<harmos_runtime::Response<()>, std::io::Error> {
    let counts = ctx.resource::<Counts>().expect("declared resource is registered");
    counts.constructed.fetch_add(1, Ordering::SeqCst);
    let _guard = AttemptGuard { counts };
    request.started.notify_one();
    if request.blocked {
        std::future::pending::<()>().await;
    }
    Ok(().into())
}

fn hold_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        HOLD_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Counts>()] },
        hold_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.counts.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime(counts: Arc<Counts>) -> Runtime<App> {
    Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Counts>(counts);
        setup.settings().operation_settings(Settings {
            queued: NonZeroUsize::new(1).unwrap(),
            lifecycle_queued: NonZeroUsize::new(1).unwrap(),
            ..Settings::default()
        });
    })
    .await
    .unwrap()
}

fn request(blocked: bool) -> HoldContractDeclaration {
    HoldContractDeclaration {
        blocked,
        started: Arc::new(Notify::new()),
    }
}

/* === Tests === */

#[tokio::test]
async fn queue_refuses_construction() {
    let counts = Arc::new(Counts::default());
    let runtime = runtime(counts.clone()).await;
    runtime.operations.register_catalog(operations_catalog(), crate::support::SERIAL).await.unwrap();
    let first_request = request(true);
    let started = first_request.started.clone();
    let first = runtime.operation(first_request).start().await.unwrap();
    started.notified().await;
    let second = runtime.operation(request(true)).start().await.unwrap();
    assert!(matches!(
        runtime.operation(request(false)).start().await,
        Err(Error::ResourceExhausted { resource: "operation admission queue" })
    ));

    runtime.shutdown().await.unwrap();
    drop((first, second));
}

#[tokio::test]
async fn queued_cancel_unbuilt() {
    let counts = Arc::new(Counts::default());
    let runtime = runtime(counts.clone()).await;
    runtime.operations.register_catalog(operations_catalog(), crate::support::SERIAL).await.unwrap();
    let first_request = request(true);
    let started = first_request.started.clone();
    let first = runtime.operation(first_request).start().await.unwrap();
    started.notified().await;
    let second_request = request(false);
    let mut second = runtime.operation(second_request).start().await.unwrap();
    assert_eq!(second.status().phase(), OperationPhase::Queued);
    second.request_cancel();
    second.request_cancel();
    assert!(second.status().cancellation_requested());
    assert_eq!(second.status().phase(), OperationPhase::Stopping);
    // A cancelled queued instance never initialized, so it owns
    // nothing to clean up and releases its queue slot at once.
    assert!(matches!((&mut second).await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(second.status().phase(), OperationPhase::Settled);
    assert_eq!(second.status().outcome(), Some(OperationOutcome::Cancelled));
    assert!(second.status().started_at().is_none());
    second.settle().await.unwrap();
    assert!(runtime.operations.status(second.id()).is_none());
    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(counts.drops.load(Ordering::SeqCst), 0);
    let third = runtime.operation(request(false)).start().await.unwrap();
    third.request_cancel();
    assert!(matches!(third.join().await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    first.request_cancel();
    assert!(matches!(first.join().await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn instances_leave_lookup() {
    let counts = Arc::new(Counts::default());
    let runtime = runtime(counts.clone()).await;
    runtime.operations.register_catalog(operations_catalog(), crate::support::SERIAL).await.unwrap();
    for _ in 0..64 {
        let instance = runtime.operation(request(false)).start().await.unwrap();
        let id = instance.id();
        instance.join().await.unwrap();
        assert!(runtime.operations.status(id).is_none());
    }
    assert_eq!(counts.drops.load(Ordering::SeqCst), 64);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn thread_submission_works() {
    let counts = Arc::new(Counts::default());
    let runtime = runtime(counts.clone()).await;
    runtime.operations.register_catalog(operations_catalog(), crate::support::SERIAL).await.unwrap();
    let operations = runtime.operations.clone();
    let instance = std::thread::spawn(move || operations.submit::<HoldContractDeclaration>(request(false))).join().unwrap().unwrap();
    instance.join().await.unwrap();
    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}
