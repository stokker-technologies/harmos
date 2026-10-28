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
    Lanes,
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

#[harmos_runtime::message]
#[derive(Clone, Copy)]
enum Mode {
    Value(u64),
    Reject,
    Wait,
    Retry,
}

#[derive(Debug, thiserror::Error)]
enum Failed {
    #[error("the instrument said no")]
    Instrument,

    #[error("not drained yet")]
    Backlog,

    #[error(transparent)]
    Child(#[from] Error),
}

#[harmos_runtime::operation(resources = [Counts, harmos_runtime::operations::HostContext], response = u64,
        id = "outcomes.compute",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 2,
                backoff: Backoff::Fixed { delay: Duration::from_millis(10), jitter: Jitter::None },
            },
        },
    )]
struct ComputeContractDeclaration {
    mode: Mode,
}

#[harmos_runtime::operation(resources = [harmos_runtime::operations::HostContext], response = u64,
        id = "outcomes.parent",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(3),
            retries: RetryPolicy::Never,
        },
    )]
struct ParentContractDeclaration {
    mode: Mode,
}

#[harmos_runtime::catalog]
enum Operations {
    ComputeContractDeclaration(ComputeContractDeclaration),
    ParentContractDeclaration(ParentContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for ComputeContractDeclaration {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Exported<harmos_runtime::Response<u64>> {
        let counts = ctx.resource::<Counts>();
        counts.started.fetch_add(1, Ordering::SeqCst);
        let _guard = AttemptGuard(counts);
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let attempt = host.borrow().attempt() + 1;
        let value = match self.mode {
            Mode::Value(value) => value,
            Mode::Reject => return Err(Failed::Instrument),
            Mode::Wait => std::future::pending().await,
            Mode::Retry if attempt < 3 => return Err(Failed::Backlog),
            Mode::Retry => u64::try_from(attempt).expect("attempts fit in u64"),
        };
        Ok(value.into())
    }

    fn classify(error: &Failed) -> FailureKind {
        match error {
            Failed::Backlog => FailureKind::Transient,
            _ => FailureKind::Permanent,
        }
    }
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        self.0.finished.fetch_add(1, Ordering::SeqCst);
    }
}

impl harmos_runtime::operations::Operation for ParentContractDeclaration {
    type Error = Failed;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Answered<harmos_runtime::operations::Response<u64>> {
        let host = context.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let answer = context.submit::<ComputeContractDeclaration>(ComputeContractDeclaration { mode: self.mode })?.await?;
            Ok(*answer)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Counts>) {
    let counts = Arc::new(Counts::default());
    let runtime = Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Counts>(counts.clone());
    })
    .await
    .unwrap();
    runtime.operations.register::<super::Single<ComputeContractDeclaration>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<super::Single<ParentContractDeclaration>>(Lanes::Serial).await.unwrap();
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
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn deadlines_remain_distinct() {
    let (runtime, _) = runtime().await;
    let elapsed = runtime.operation(ComputeContractDeclaration { mode: Mode::Wait }).start().await.unwrap().await;
    assert!(matches!(elapsed, Err(Error::OperationRefused(Refusal::TimedOut))));

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
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Child(Error::OperationRefused(Refusal::TimedOut)))));
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn child_retries_settle_each_attempt() {
    let (runtime, counts) = runtime().await;
    let root = runtime.operation(ComputeContractDeclaration { mode: Mode::Retry }).start().await.unwrap().await.unwrap();
    let child = runtime.operation(ParentContractDeclaration { mode: Mode::Retry }).start().await.unwrap().await.unwrap();
    assert_eq!((*root, *child), (3, 3));
    assert_eq!(counts.started.load(Ordering::SeqCst), 6);
    assert_eq!(counts.finished.load(Ordering::SeqCst), 6);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failure_never_emit() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.resource(Counts::default());
        setup.operations::<super::Single<ComputeContractDeclaration>>(Lanes::Open);
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
    runtime.shutdown().await.unwrap();
}
