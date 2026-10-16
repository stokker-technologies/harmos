/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

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
use harmos::operations::{
    Backoff, Context, FailureKind, Jitter, Operation, OperationPolicy, RestartPolicy, RetryPolicy,
};
use harmos::{Error, Refusal, Runtime, state::FactSource, state::Position, state::SystemFact};

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
#[derive(Clone, Copy)]
enum Mode {
    Value(usize),
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

#[harmos::operation(
        request = Mode,
        response = usize,
        id = "outcomes.compute",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::OnTransient {
                max_retries: 2,
                backoff: Backoff::Fixed { delay: Duration::from_millis(10), jitter: Jitter::None },
            },
            restart: RestartPolicy::Never,
        },
    )]
struct ComputeContract;

#[harmos::operation(
        request = Mode,
        response = usize,
        id = "outcomes.parent",
        policy = OperationPolicy {
            timeout: Duration::from_secs(3),
            attempt_timeout: Duration::from_secs(3),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct ParentContract;

#[harmos::catalog]
enum Operations {
    ComputeContract(ComputeContract),
    ParentContract(ParentContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Compute {
    type Declaration = ComputeContract;
    type Resources = Counts;
    type Error = Failed;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(
        counts: Arc<Counts>,
        _context: Self::Context<'ctx>,
    ) -> Exported<Self> {
        let constructed: Exported<Self> = async move {
            counts.constructed.fetch_add(1, Ordering::SeqCst);
            Ok(Self {
                attempts: 0,
                counts,
            })
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

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        mode: &mut Mode,
        _endpoints: &mut Self::Endpoints,
    ) -> Exported<harmos::operations::Response<usize>> {
        let result: ::core::result::Result<usize, Self::Error> = async move {
            assert_eq!(context.attempt(), self.attempts);
            self.attempts += 1;
            match mode {
                Mode::Value(value) => Ok(*value),
                Mode::Reject => Err(Failed::Instrument),
                Mode::Wait => std::future::pending().await,
                Mode::Retry if self.attempts < 3 => Err(Failed::Backlog),
                Mode::Retry => Ok(self.attempts),
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
    type Declaration = ParentContract;
    type Resources = ();
    type Error = Failed;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_resources: Arc<()>, _: Self::Context<'ctx>) -> Answered<Self> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        mode: &mut Mode,
        _endpoints: &mut Self::Endpoints,
    ) -> Answered<harmos::operations::Response<usize>> {
        let result: ::core::result::Result<usize, Self::Error> = async move {
            let answer = context.submit_as::<ComputeContract>(*mode)?.await?;
            Ok(*answer)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> (Runtime<App>, Arc<Counts>) {
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    let counts = Arc::new(Counts::default());
    runtime
        .operations
        .register_operation::<Compute>("app", Lanes::Open, counts.clone())
        .unwrap();
    runtime
        .operations
        .register_operation::<Parent>("app", Lanes::Serial, Arc::new(()))
        .unwrap();
    (runtime, counts)
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("aliases_preserve_values", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, _) = runtime().await;
                    let root = runtime
                        .operations
                        .submit_as::<ComputeContract>(Mode::Value(4))
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(*root, 4);
                    let child = runtime
                        .operations
                        .submit_as::<ParentContract>(Mode::Value(9))
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(*child, 9);
                    let error = runtime
                        .operations
                        .submit_as::<ComputeContract>(Mode::Reject)
                        .unwrap()
                        .await
                        .unwrap_err();
                    let Error::OperationFailed { operation, source } = error else {
                        panic!("expected authored failure")
                    };
                    assert_eq!(operation, "outcomes.compute");
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Instrument)
                    ));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("deadlines_remain_distinct", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, _) = runtime().await;
                    let elapsed = runtime
                        .operations
                        .submit_as::<ComputeContract>(Mode::Wait)
                        .unwrap()
                        .await;
                    assert!(matches!(
                        elapsed,
                        Err(Error::OperationRefused(Refusal::TimedOut))
                    ));

                    let error = runtime
                        .operations
                        .submit_as::<ParentContract>(Mode::Reject)
                        .unwrap()
                        .await
                        .unwrap_err();
                    let Error::OperationFailed { operation, source } = error else {
                        panic!("expected parent failure")
                    };
                    assert_eq!(operation, "outcomes.parent");
                    let Some(Failed::Child(Error::OperationFailed { operation, source })) =
                        source.downcast_ref::<Failed>()
                    else {
                        panic!("expected original child failure")
                    };
                    assert_eq!(operation, "outcomes.compute");
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Instrument)
                    ));

                    let error = runtime
                        .operations
                        .submit_as::<ParentContract>(Mode::Wait)
                        .unwrap()
                        .await
                        .unwrap_err();
                    let Error::OperationFailed { source, .. } = error else {
                        panic!("expected parent failure")
                    };
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Child(Error::OperationRefused(Refusal::TimedOut)))
                    ));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("child_retries_reuse", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (runtime, counts) = runtime().await;
                    let root = runtime
                        .operations
                        .submit_as::<ComputeContract>(Mode::Retry)
                        .unwrap()
                        .await
                        .unwrap();
                    let child = runtime
                        .operations
                        .submit_as::<ParentContract>(Mode::Retry)
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!((*root, *child), (3, 3));
                    assert_eq!(counts.constructed.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.initialized.load(Ordering::SeqCst), 2);
                    assert_eq!(counts.finalized.load(Ordering::SeqCst), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("failure_never_emit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .operation::<Compute>("app", Lanes::Open, Arc::new(Counts::default()))
                        .runtime()
                        .await
                        .unwrap();
                    let error = runtime
                        .operations
                        .submit_as::<ComputeContract>(Mode::Reject)
                        .unwrap()
                        .await
                        .unwrap_err();
                    let Error::OperationFailed { source, .. } = error else {
                        panic!("expected authored failure")
                    };
                    assert!(matches!(
                        source.downcast_ref::<Failed>(),
                        Some(Failed::Instrument)
                    ));
                    let facts: Vec<_> = runtime
                        .state
                        .query::<SystemFact>(Position::ORIGIN)
                        .await
                        .map(|entry| entry.expect("system evidence is readable"))
                        .filter(|entry| matches!(entry.value.source, FactSource::Work(_)))
                        .collect();
                    assert!(facts.is_empty());
                    runtime.stop().await.unwrap();
                });
        });
    }
}
