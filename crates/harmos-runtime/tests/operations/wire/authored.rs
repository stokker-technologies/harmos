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
use harmos_runtime::{Error, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Trace {
    entered: AtomicUsize,
    dropped: AtomicUsize,
    running: Notify,
}

struct AttemptGuard(Arc<Trace>);

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
struct Answer {
    value: u64,
}

#[derive(harmos_runtime::Message, Debug)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = Answer)]
#[derive(Clone)]
struct ReadContractDeclaration {
    mode: Mode,
}

static READ_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ReadContractDeclaration>(
        "authored.read",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: harmos_runtime::RetryPolicy::Never,
        },
    );

const READ_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ReadContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ReadContractDeclaration>::new(&READ_CONTRACT_DECLARATION_DESCRIPTION);

/* === Enums === */

#[derive(harmos_runtime::Message, Clone, Debug, Default)]
enum Mode {
    #[default]
    Complete,
    Wait,
    Panic,
}

/* === Traits === */

/* === Implementations === */

async fn read_contract_declaration_handler(
    request: ReadContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<ReadContractDeclaration>,
) -> Result<harmos_runtime::Response<Answer>, std::convert::Infallible> {
    let trace = ctx.resource::<Trace>().expect("declared resource is registered");
    let _guard = AttemptGuard(trace.clone());
    trace.entered.fetch_add(1, Ordering::SeqCst);
    trace.running.notify_one();
    match request.mode {
        Mode::Complete => Ok(Answer { value: 7 }.into()),
        Mode::Wait => std::future::pending().await,
        Mode::Panic => panic!("scripted attempt panic"),
    }
}

fn read_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        READ_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Trace>()] },
        read_contract_declaration_handler,
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
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Trace>(trace.clone()).operation(
            read_contract_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        );
    })
    .await
    .unwrap();
    (runtime, trace)
}

#[tokio::test]
async fn authored_request_completes_cancels_and_panics() {
    // Register Without Executing
    let (runtime, trace) = runtime().await;
    assert_eq!(trace.entered.load(Ordering::SeqCst), 0);

    // Complete And Release Attempt State
    let response = runtime.operation(ReadContractDeclaration { mode: Mode::Complete }).await.unwrap();
    assert_eq!(response.value, 7);
    assert_eq!(trace.entered.load(Ordering::SeqCst), 1);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 1);
    trace.running.notified().await;

    // Cancel A Running Request And Release Its State
    let call = runtime.operation(ReadContractDeclaration { mode: Mode::Wait }).start().await.unwrap();
    trace.running.notified().await;
    call.request_cancel();
    assert!(call.await.is_err());
    assert_eq!(trace.entered.load(Ordering::SeqCst), 2);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 2);

    // Contain A Panic And Release Its State
    let result = runtime.operation(ReadContractDeclaration { mode: Mode::Panic }).await;
    assert!(matches!(result, Err(Error::OperationPanicked { .. })));
    assert_eq!(trace.entered.load(Ordering::SeqCst), 3);
    assert_eq!(trace.dropped.load(Ordering::SeqCst), 3);

    // Stop
    crate::support::app::shutdown(&runtime).await.unwrap();
}
