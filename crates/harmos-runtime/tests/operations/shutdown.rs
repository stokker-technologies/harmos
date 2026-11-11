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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::app::App;
use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Trace {
    started: Notify,
    release: Notify,
    counted: AtomicUsize,
    dropped: AtomicUsize,
}

struct AttemptGuard(Arc<Trace>);

/* === Enums === */

#[derive(harmos_runtime::Message, Clone, Debug)]
enum Mode {
    Hold,
    Count,
    Sleep,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Hold
    }
}

#[derive(harmos_runtime::Message, Debug)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = ())]
#[derive(Clone)]
struct WaitContractDeclaration {
    mode: Mode,
}

static WAIT_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<WaitContractDeclaration>(
        "shutdown.wait",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(2),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const WAIT_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<WaitContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<WaitContractDeclaration>::new(&WAIT_CONTRACT_DECLARATION_DESCRIPTION);

fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(wait_contract_declaration_operation().expect("representable operation"));
    batch
}

/* === Traits === */

/* === Implementations === */

async fn wait_contract_declaration_handler(
    request: WaitContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<WaitContractDeclaration>,
) -> Result<harmos_runtime::Response<()>, std::convert::Infallible> {
    let trace = ctx.resource::<Trace>().expect("declared resource is registered");
    let _guard = AttemptGuard(trace.clone());
    match request.mode {
        Mode::Hold => {
            trace.started.notify_one();
            trace.release.notified().await;
        }
        Mode::Count => {
            trace.counted.fetch_add(1, Ordering::SeqCst);
        }
        Mode::Sleep => {
            trace.started.notify_one();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    Ok(().into())
}

fn wait_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        WAIT_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>()] },
        wait_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Trace>) {
    let trace = Arc::new(Trace::default());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Trace>(trace.clone());
    })
    .await
    .unwrap();
    runtime.operations.register_catalog(operations_catalog(), crate::support::SERIAL).await.unwrap();
    (runtime, trace)
}

/* === Tests === */

#[tokio::test]
async fn cancellation_observable_queued() {
    let (runtime, trace) = runtime().await;
    let first = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    let queued = runtime.operation(WaitContractDeclaration { mode: Mode::Count }).start().await.unwrap();
    queued.request_cancel();
    assert!(matches!(queued.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(trace.counted.load(Ordering::SeqCst), 0);
    trace.release.notify_one();
    first.await.unwrap();

    let running = runtime.operation(WaitContractDeclaration { mode: Mode::Sleep }).start().await.unwrap();
    trace.started.notified().await;

    running.request_cancel();
    assert!(matches!(running.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));

    // The cancelled queued instance was never constructed.
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_cancels_attempts() {
    let (runtime, trace) = runtime().await;
    let instance = runtime.operation(WaitContractDeclaration { mode: Mode::Hold }).start().await.unwrap();
    trace.started.notified().await;
    runtime.shutdown().await.unwrap();
    assert!(matches!(instance.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    assert!(matches!(runtime.operations.submit_resident("late").await, Err(Error::Closing { domain: "resident" })));
}
