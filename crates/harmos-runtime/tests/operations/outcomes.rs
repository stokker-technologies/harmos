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
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, Operation, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime, state::FactSource, state::Position, state::SystemFact};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Exported<T> = Result<T, Failed>;
type Answered<T, E = Failed> = Result<T, E>;

/* === Structs === */

#[derive(Default)]
struct Counts {
    constructed: AtomicUsize,
    initialized: AtomicUsize,
    finalized: AtomicUsize,
}

struct Compute {
    attempts: usize,
    counts: Arc<Counts>,
}

struct Parent;

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

#[harmos_runtime::operation(request = ComputeContract, infer_request,
        response = u64,
        id = "outcomes_compute",

        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 2,
                backoff: Backoff::Fixed { delay: Duration::from_millis(10), jitter: Jitter::None },
            },
        },
    )]
struct ComputeContractDeclaration;

#[harmos_runtime::message]

struct ComputeContract {
    mode: Mode,
}

#[harmos_runtime::operation(request = ParentContract, infer_request,
        response = u64,
        id = "outcomes_parent",

        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(3),
            retries: RetryPolicy::Never,
        },
    )]
struct ParentContractDeclaration;

#[harmos_runtime::message]

struct ParentContract {
    mode: Mode,
}

#[harmos_runtime::catalog]
enum Operations {
    ComputeContract(ComputeContractDeclaration),
    ParentContract(ParentContractDeclaration),
}

/* === Traits === */

/* === Implementations === */

impl Operation for Compute {
    type Request = ComputeContract;
    type Response = <ComputeContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Failed;
    type Endpoints = ();

    async fn initialize(_context: harmos_runtime::OperationContext) -> Exported<Self> {
        let counts = _context.dependency::<Counts>().expect("fixture registers its dependency");
        let host = _context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let _context = host.borrow();
        let constructed: Exported<Self> = async move {
            counts.constructed.fetch_add(1, Ordering::SeqCst);
            Ok(Self { attempts: 0, counts })
        }
        .await;
        let mut operation = constructed?;
        let started: Exported<()> = async {
            let operation = &mut operation;
            operation.counts.initialized.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        .await;
        started?;
        Ok(operation)
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut ComputeContract, _endpoints: &mut Self::Endpoints,
    ) -> Exported<harmos_runtime::operations::Response<u64>> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            assert_eq!(context.attempt(), self.attempts);
            self.attempts += 1;
            match request.mode {
                Mode::Value(value) => Ok(value),
                Mode::Reject => Err(Failed::Instrument),
                Mode::Wait => std::future::pending().await,
                Mode::Retry if self.attempts < 3 => Err(Failed::Backlog),
                Mode::Retry => Ok(u64::try_from(self.attempts).expect("attempts fit in u64")),
            }
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    fn classify(error: &Failed) -> FailureKind {
        match error {
            Failed::Backlog => FailureKind::Transient,
            _ => FailureKind::Permanent,
        }
    }

    async fn cleanup(&mut self) -> Exported<()> {
        self.counts.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl Operation for Parent {
    type Request = ParentContract;
    type Response = <ParentContractDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Failed;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Answered<Self> {
        let _resources = context.dependency::<()>().expect("fixture registers its dependency");
        Ok(Self)
    }

    async fn run(
        &mut self, context: harmos_runtime::OperationContext, request: &mut ParentContract, _endpoints: &mut Self::Endpoints,
    ) -> Answered<harmos_runtime::operations::Response<u64>> {
        let host = context.dependency::<harmos_runtime::operations::HostContext>().expect("native runtime context");
        let context = host.borrow();
        let result: ::core::result::Result<u64, Self::Error> = async move {
            let answer = context.submit::<ComputeContractDeclaration>(ComputeContract { mode: request.mode })?.await?;
            Ok(*answer)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn runtime() -> (Runtime<App>, Arc<Counts>) {
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    let counts = Arc::new(Counts::default());
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(counts.clone()).unwrap();
                dependencies
            })
            .implementation::<ComputeContractDeclaration, Compute>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(())).unwrap();
                dependencies
            })
            .implementation::<ParentContractDeclaration, Parent>(),
            Lanes::Serial,
        ))
        .await
        .unwrap();
    (runtime, counts)
}

/* === Tests === */

#[tokio::test]
async fn aliases_preserve_values() {
    let (runtime, _) = runtime().await;
    let root = runtime.operation(ComputeContract { mode: Mode::Value(4) }).start().await.unwrap().await.unwrap();
    assert_eq!(*root, 4);
    let child = runtime.operation(ParentContract { mode: Mode::Value(9) }).start().await.unwrap().await.unwrap();
    assert_eq!(*child, 9);
    let error = runtime.operation(ComputeContract { mode: Mode::Reject }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { operation, source } = error else { panic!("expected authored failure") };
    assert_eq!(operation, "outcomes_compute");
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Instrument)));
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn deadlines_remain_distinct() {
    let (runtime, _) = runtime().await;
    let elapsed = runtime.operation(ComputeContract { mode: Mode::Wait }).start().await.unwrap().await;
    assert!(matches!(elapsed, Err(Error::OperationRefused(Refusal::TimedOut))));

    let error = runtime.operation(ParentContract { mode: Mode::Reject }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { operation, source } = error else { panic!("expected parent failure") };
    assert_eq!(operation, "outcomes_parent");
    let Some(Failed::Child(Error::OperationFailed { operation, source })) = source.downcast_ref::<Failed>() else {
        panic!("expected original child failure")
    };
    assert_eq!(operation, "outcomes_compute");
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Instrument)));

    let error = runtime.operation(ParentContract { mode: Mode::Wait }).start().await.unwrap().await.unwrap_err();
    let Error::OperationFailed { source, .. } = error else { panic!("expected parent failure") };
    assert!(matches!(source.downcast_ref::<Failed>(), Some(Failed::Child(Error::OperationRefused(Refusal::TimedOut)))));
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn child_retries_reuse() {
    let (runtime, counts) = runtime().await;
    let root = runtime.operation(ComputeContract { mode: Mode::Retry }).start().await.unwrap().await.unwrap();
    let child = runtime.operation(ParentContract { mode: Mode::Retry }).start().await.unwrap().await.unwrap();
    assert_eq!((*root, *child), (3, 3));
    assert_eq!(counts.constructed.load(Ordering::SeqCst), 2);
    assert_eq!(counts.initialized.load(Ordering::SeqCst), 2);
    assert_eq!(counts.finalized.load(Ordering::SeqCst), 2);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn failure_never_emit() {
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .with_registration(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(Arc::new(Counts::default())).unwrap();
                dependencies
            })
            .implementation::<ComputeContractDeclaration, Compute>(),
            Lanes::Open,
        ))
        .runtime()
        .await
        .unwrap();
    let error = runtime.operation(ComputeContract { mode: Mode::Reject }).start().await.unwrap().await.unwrap_err();
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
    runtime.stop().await.unwrap();
}
