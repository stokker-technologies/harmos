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

// Local Imports
use super::{
    app::App,
    support::{Bank, Ledger, catalog},
};
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime, state::FactSource, state::Position, state::SystemFact};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Exported<T> = Result<T, Failed>;
type Answered<T, E = Failed> = Result<T, E>;

/* === Structs === */

#[derive(Default)]
struct Counts {
    started: AtomicUsize,
    finished: AtomicUsize,
}

struct AttemptGuard(Arc<Counts>);

/* === Enums === */

#[derive(harmos_runtime::Message, Debug, Clone, Copy)]
enum Mode {
    Value(u64),
    Reject,
    Wait,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Value(Default::default())
    }
}

#[derive(Debug, thiserror::Error)]
enum Failed {
    #[error("the instrument said no")]
    Instrument,

    #[error(transparent)]
    Child(#[from] Error),
}

#[derive(harmos_runtime::Message, Debug)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct ComputeContractDeclaration {
    mode: Mode,
}

static COMPUTE_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ComputeContractDeclaration>(
        "outcomes.compute",
        OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 2,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(10),
                    jitter: Jitter::None,
                },
            },
        },
    );

const COMPUTE_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ComputeContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ComputeContractDeclaration>::new(&COMPUTE_CONTRACT_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Message, Debug)]
#[harmos_path(harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[request(response = u64)]
#[derive(Clone)]
struct ParentContractDeclaration {
    mode: Mode,
}

static PARENT_CONTRACT_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ParentContractDeclaration>(
        "outcomes.parent",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(3),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::Never,
        },
    );

const PARENT_CONTRACT_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ParentContractDeclaration> =
    harmos_runtime::operations::OperationSpec::<ParentContractDeclaration>::new(&PARENT_CONTRACT_DECLARATION_DESCRIPTION);

/* === Traits === */

/* === Implementations === */

async fn compute_contract_declaration_handler(
    request: ComputeContractDeclaration, ctx: harmos_runtime::operations::FunctionContext<ComputeContractDeclaration>,
) -> Exported<harmos_runtime::Response<u64>> {
    let counts = ctx.resource::<Counts>().expect("declared resource is registered");
    counts.started.fetch_add(1, Ordering::SeqCst);
    let _guard = AttemptGuard(counts);
    let value = match request.mode {
        Mode::Value(value) => value,
        Mode::Reject => return Err(Failed::Instrument),
        Mode::Wait => std::future::pending().await,
    };
    Ok(value.into())
}

fn compute_contract_declaration_classification(error: &Failed) -> FailureKind {
    let _ = error;
    FailureKind::Permanent
}

fn compute_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        COMPUTE_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Counts>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        compute_contract_declaration_handler,
        compute_contract_declaration_classification,
    )
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::SeqCst);
    }
}

async fn parent_contract_declaration_handler(
    request: ParentContractDeclaration, context: harmos_runtime::operations::FunctionContext<ParentContractDeclaration>,
) -> Answered<harmos_runtime::operations::Response<u64>> {
    let host = context.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    let result: ::core::result::Result<u64, Failed> = async move {
        let answer = context.submit::<ComputeContractDeclaration>(ComputeContractDeclaration { mode: request.mode })?.await?;
        Ok(*answer)
    }
    .await;
    result.map(::core::convert::Into::into)
}

fn parent_contract_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PARENT_CONTRACT_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        parent_contract_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Counts>) {
    let counts = Arc::new(Counts::default());
    let runtime = crate::support::app::launch::<App>((), (), |setup| {
        setup.resource::<Counts>(counts.clone());
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(
            compute_contract_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    runtime
        .operations
        .register_operation(parent_contract_declaration_operation().expect("representable operation"), crate::support::SERIAL)
        .await
        .unwrap();
    (runtime, counts)
}

/* === Tests === */

#[tokio::test]
async fn aliases_preserve_values() {
    let (runtime, _) = runtime().await;
    let root = runtime.operation(ComputeContractDeclaration { mode: Mode::Value(4) }).start().await.unwrap().await.unwrap();
    assert_eq!(*root, 4);
    let child = runtime.operation(ParentContractDeclaration { mode: Mode::Value(9) }).start().await.unwrap().await.unwrap();
    assert_eq!(*child, 9);
    let error = runtime.operation(ComputeContractDeclaration { mode: Mode::Reject }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { operation, source } = error else { panic!("expected authored failure") };
    assert_eq!(operation, "outcomes.compute");
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Instrument)));
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn deadlines_remain_distinct() {
    let (runtime, _) = runtime().await;
    let elapsed = runtime.operation(ComputeContractDeclaration { mode: Mode::Wait }).start().await.unwrap().await;
    assert!(matches!(elapsed, Err(Error::OperationRefused(Refusal::TimedOut { .. }))));

    let error = runtime.operation(ParentContractDeclaration { mode: Mode::Reject }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { operation, source } = error else { panic!("expected parent failure") };
    assert_eq!(operation, "outcomes.parent");
    let Some(Failed::Child(Error::OperationFailed { operation, source })) = source.downcast_ref::<Failed>() else {
        panic!("expected original child failure")
    };
    assert_eq!(operation, "outcomes.compute");
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Instrument)));

    let error = runtime.operation(ParentContractDeclaration { mode: Mode::Wait }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected parent failure") };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Child(Error::OperationRefused(Refusal::TimedOut { .. })))));
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn failure_never_emit() {
    let runtime = crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.resource(Counts::default());
        setup.operation(
            compute_contract_declaration_operation().expect("representable operation"),
            harmos_runtime::operations::Lane::default(),
        );
    })
    .await
    .unwrap();
    let error = runtime.operation(ComputeContractDeclaration { mode: Mode::Reject }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected authored failure") };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Instrument)));
    let facts: Vec<_> = runtime
        .state
        .query::<SystemFact>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("system evidence is readable"))
        .filter(|entry| matches!(entry.value.source, FactSource::Work(_)))
        .collect();
    assert!(facts.is_empty());
    crate::support::app::shutdown(&runtime).await.unwrap();
}
