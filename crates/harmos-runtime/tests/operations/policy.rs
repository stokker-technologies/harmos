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
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::Notify;
// Local Imports
use super::{Lanes, runtime_with};
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationDeclaration, OperationPolicy, ReplaySafety, RetryPolicy};
use harmos_runtime::{Error, Refusal};

/* === Constants === */

/* === Macros === */

// Each Request Executes Its Scripted Policy With Declared Resources.
macro_rules! behaves {
    ($($ty:ident),* $(,)?) => {$(
        super::in_process!($ty);

        impl harmos_runtime::MemorySize for $ty {
            fn memory_size(&self) -> usize {
                std::mem::size_of::<Self>()
            }
        }

        impl From<Behavior> for $ty {
            fn from(behavior: Behavior) -> Self {
                Self { behavior }
            }
        }

        impl harmos_runtime::Operation for $ty {
            type Error = std::io::Error;

            async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
                attempt(self, ctx).await
            }

            fn classify(error: &Self::Error) -> FailureKind {
                match error.kind() {
                    std::io::ErrorKind::Interrupted => FailureKind::Transient,
                    std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
                    _ => FailureKind::Permanent,
                }
            }
        }

        impl Behaves for $ty {
            fn behavior(&self) -> &Behavior {
                &self.behavior
            }
        }
    )*};
}

macro_rules! scripted {
    ($name:ident, $overall:expr, $attempt:expr, $retries:expr, $delay:expr, $replay:expr) => {
        #[harmos_runtime::operation(
                                                            resources = [Owner, harmos_runtime::operations::HostContext],
                                                            codec = manual,
                                                            id = "policy.compute",
                                                            response = u64,
                                                            policy = policy($overall, $attempt, $retries, $delay),
                                                            replay = if $replay { ReplaySafety::Idempotent } else { ReplaySafety::Unknown },
                                                        )]
        #[derive(Clone)]
        struct $name {
            behavior: Behavior,
        }

        behaves!($name);
    };
}

/* === Type Aliases === */

/* === Structs === */

// Each Request Carries Its Scripted Behavior, Which Can Hold A Shared Gate,
// So Every Policy Request Stays In Process.
#[harmos_runtime::operation(resources = [BlockerOwner], codec = manual, id = "policy.blocker",
    response = u64,
    policy = policy(30_000, 30_000, 0, 1)
)]
#[derive(Clone)]
struct BlockerDeclaration {
    behavior: Behavior,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], codec = manual, id = "policy.exponential",
    response = u64,
    policy = OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::OnTransient {
            max_retries: 4,
            backoff: Backoff::Exponential {
                initial: Duration::from_millis(200),
                maximum: Duration::from_secs(1),
                jitter: Jitter::None,
            },
        },
    }
)]
#[derive(Clone)]
struct ExponentialDeclaration {
    behavior: Behavior,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], codec = manual, id = "policy.jittered",
    response = u64,
    policy = OperationPolicy {
        retries: RetryPolicy::OnTransient {
            max_retries: 4,
            backoff: Backoff::Exponential {
                initial: Duration::from_millis(200),
                maximum: Duration::from_secs(1),
                jitter: Jitter::Deterministic,
            },
        },
        ..ExponentialDeclaration::POLICY
    }
)]
#[derive(Clone)]
struct JitteredDeclaration {
    behavior: Behavior,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], codec = manual, id = "policy.empty",
    response = u64,
    policy = OperationPolicy {
        retries: RetryPolicy::OnTransient {
            max_retries: 0,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
        ..BlockerDeclaration::POLICY
    }
)]
#[derive(Clone)]
struct EmptyRetriesDeclaration {
    behavior: Behavior,
}

#[harmos_runtime::operation(resources = [Owner, harmos_runtime::operations::HostContext], codec = manual, id = "policy.inverted",
    response = u64,
    policy = OperationPolicy {
        retries: RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: Backoff::Exponential {
                initial: Duration::from_millis(100),
                maximum: Duration::from_millis(50),
                jitter: Jitter::None,
            },
        },
        ..BlockerDeclaration::POLICY
    }
)]
#[derive(Clone)]
struct InvertedBackoffDeclaration {
    behavior: Behavior,
}

#[derive(Default)]
struct Probe {
    attempts: AtomicUsize,
    times: Mutex<Vec<tokio::time::Instant>>,
    started: Notify,
}

struct Owner(Arc<Probe>);

struct BlockerOwner(Arc<Probe>);

/* === Enums === */

#[derive(Clone)]
enum Behavior {
    Fail { remaining: usize, kind: std::io::ErrorKind },
    Pending,
    Gate(Arc<Notify>),
}

/* === Traits === */

trait Behaves:
    harmos_runtime::Operation<Error = std::io::Error>
    + OperationDeclaration<
        Request = Self,
        Response = u64,
        Input = harmos_runtime::operations::NoStream,
        Output = harmos_runtime::operations::NoStream,
    > + harmos_runtime::operations::Requires<Owner>
    + harmos_runtime::operations::Requires<harmos_runtime::operations::HostContext>
    + From<Behavior>
    + Sync
{
    fn behavior(&self) -> &Behavior;
}

/* === Implementations === */

behaves!(ExponentialDeclaration, JitteredDeclaration, EmptyRetriesDeclaration, InvertedBackoffDeclaration);

super::in_process!(BlockerDeclaration);

impl harmos_runtime::MemorySize for BlockerDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl harmos_runtime::Operation for BlockerDeclaration {
    type Error = std::io::Error;

    async fn run(self, context: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
        let probe = context.resource::<BlockerOwner>();
        probe.0.attempts.fetch_add(1, Ordering::SeqCst);
        probe.0.started.notify_one();
        let Behavior::Gate(gate) = self.behavior else { unreachable!("the blocker owns a release gate") };
        gate.notified().await;
        Ok(1.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

const fn policy(overall: u64, attempt: u64, retries: usize, delay: u64) -> OperationPolicy {
    OperationPolicy {
        timeout: Duration::from_millis(overall),
        attempt_timeout: Duration::from_millis(attempt),
        retries: if retries == 0 {
            RetryPolicy::Never
        } else {
            RetryPolicy::OnTransient {
                max_retries: retries,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(delay),
                    jitter: Jitter::None,
                },
            }
        },
    }
}

async fn attempt<O: Behaves>(request: O, ctx: harmos_runtime::Context<'_, O>) -> std::io::Result<harmos_runtime::Response<u64>> {
    let resources = ctx.resource::<Owner>();
    let host = ctx.resource::<harmos_runtime::operations::HostContext>();
    let context = host.borrow();
    resources.0.attempts.fetch_add(1, Ordering::SeqCst);
    resources.0.times.lock().unwrap().push(context.scope().now());
    resources.0.started.notify_one();
    match request.behavior() {
        Behavior::Fail { remaining, kind } if context.attempt() < *remaining => {
            return Err(std::io::Error::new(*kind, "scripted attempt failure"));
        }
        Behavior::Pending => std::future::pending::<()>().await,
        Behavior::Gate(gate) => gate.notified().await,
        _ => {}
    }
    Ok(u64::try_from(context.attempt() + 1).expect("attempts fit in u64").into())
}

fn failing<O: Behaves>(remaining: usize) -> O {
    O::from(Behavior::Fail {
        remaining,
        kind: std::io::ErrorKind::Interrupted,
    })
}

fn waits(probe: &Probe) -> Vec<u64> {
    probe.times.lock().unwrap().windows(2).map(|pair| u64::try_from((pair[1] - pair[0]).as_millis()).unwrap()).collect()
}

async fn bound<O: Behaves>() -> (harmos_runtime::Runtime<super::support::Bank>, Arc<Probe>) {
    bound_with::<O>(|_| {}).await
}

async fn bound_with<O: Behaves>(
    update: impl FnOnce(&mut harmos_runtime::Setup<super::support::Bank>),
) -> (harmos_runtime::Runtime<super::support::Bank>, Arc<Probe>) {
    let probe = Arc::new(Probe::default());
    let runtime = runtime_with(|setup| {
        setup.resource(Owner(probe.clone()));
        update(setup);
    })
    .await;
    runtime.operations.register::<super::Single<O>>(Lanes::Serial).await.unwrap();
    (runtime, probe)
}

async fn invalid<O: Behaves>() {
    let probe = Arc::new(Probe::default());
    let runtime = runtime_with(|setup| {
        setup.resource(Owner(probe.clone()));
    })
    .await;
    let error = runtime.operations.register::<super::Single<O>>(Lanes::Serial).await.unwrap_err();
    assert!(matches!(error, Error::OperationDeclaration { .. }));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 0);
    assert!(runtime.operations.definition(<O as OperationDeclaration>::ID).is_err());
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn queued_deadline_starts() {
    scripted!(Queued, 50, 1000, 0, 1, false);
    let blocker_probe = Arc::new(Probe::default());
    let (runtime, queued_probe) = bound_with::<Queued>(|setup| {
        setup.resource(BlockerOwner(blocker_probe.clone()));
    })
    .await;
    runtime.operations.register::<super::Single<BlockerDeclaration>>(Lanes::Serial).await.unwrap();
    let gate = Arc::new(Notify::new());
    let blocker = runtime
        .operation(BlockerDeclaration {
            behavior: Behavior::Gate(Arc::clone(&gate)),
        })
        .start()
        .await
        .unwrap();
    blocker_probe.started.notified().await;
    let queued = runtime.operation(failing::<Queued>(0)).start().await.unwrap();
    tokio::time::advance(Duration::from_millis(100)).await;
    assert!(matches!(queued.await, Err(Error::OperationRefused(Refusal::TimedOut))));
    // An Expired Queued Request Never Starts An Attempt.
    assert_eq!(queued_probe.attempts.load(Ordering::SeqCst), 0);
    gate.notify_one();
    assert_eq!(*blocker.await.unwrap(), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn attempt_limits_backoff() {
    scripted!(Attempt, 100, 2000, 0, 1, false);
    let (runtime, probe) = bound::<Attempt>().await;
    let submitted = tokio::time::Instant::now();
    let result = runtime.operation(Attempt { behavior: Behavior::Pending }).run().await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(submitted.elapsed(), Duration::from_millis(100));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();

    scripted!(Retrying, 100, 1000, 10, 1000, false);
    let (runtime, probe) = bound::<Retrying>().await;
    let submitted = tokio::time::Instant::now();
    let result = runtime.operation(failing::<Retrying>(100)).run().await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(submitted.elapsed(), Duration::from_millis(100));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn retry_reuses_state() {
    scripted!(Retrying, 1000, 100, 2, 20, false);
    let (runtime, probe) = bound::<Retrying>().await;
    assert_eq!(*runtime.operation(failing::<Retrying>(2)).run().await.unwrap(), 3);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 3);
    assert_eq!(waits(&probe), [20, 20]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn reaches_wait_ceiling() {
    let (runtime, probe) = bound::<ExponentialDeclaration>().await;
    let result = runtime.operation(failing::<ExponentialDeclaration>(100)).run().await;
    let Err(Error::OperationFailed { source, .. }) = result else { panic!("the last typed attempt failure is retained") };
    assert_eq!(source.downcast_ref::<std::io::Error>().unwrap().kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(waits(&probe), [200, 400, 800, 1000]);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 5);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn cancellation_skips_backoff() {
    scripted!(Retrying, 10000, 100, 2, 1000, false);
    let (runtime, probe) = bound::<Retrying>().await;
    let invocation = runtime.operation(failing::<Retrying>(100)).start().await.unwrap();
    probe.started.notified().await;
    let cancelled = tokio::time::Instant::now();
    invocation.cancel();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(cancelled.elapsed(), Duration::ZERO);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn timeouts_require_replay() {
    scripted!(Unknown, 1000, 5, 1, 1, false);
    let (runtime, probe) = bound::<Unknown>().await;
    let result = runtime.operation(Unknown { behavior: Behavior::Pending }).run().await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();

    scripted!(Replayable, 1000, 5, 1, 1, true);
    let (runtime, probe) = bound::<Replayable>().await;
    let result = runtime.operation(Replayable { behavior: Behavior::Pending }).run().await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(waits(&probe), [6]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn policy_refuses_binding() {
    scripted!(Invalid0, 0, 1, 0, 1, false);
    invalid::<Invalid0>().await;
    scripted!(Invalid1, 10, 0, 0, 1, false);
    invalid::<Invalid1>().await;
    scripted!(Invalid2, 10, 5, 1, 0, false);
    invalid::<Invalid2>().await;
    invalid::<EmptyRetriesDeclaration>().await;
    invalid::<InvertedBackoffDeclaration>().await;
}

#[tokio::test]
async fn calls_settle_retry() {
    scripted!(Queued, 1000, 1000, 0, 1, false);
    let (runtime, probe) = bound::<Queued>().await;
    let gate = Arc::new(Notify::new());
    let first = runtime.operation(Queued { behavior: Behavior::Gate(gate) }).start().await.unwrap();
    probe.started.notified().await;
    let second = runtime.operation(failing::<Queued>(0)).start().await.unwrap();
    second.cancel();
    assert!(matches!(second.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    first.cancel();
    assert!(matches!(first.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    // A Cancelled Queued Request Never Starts An Attempt.
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn jitter_respects_ceiling() {
    let (runtime, probe) = bound::<JitteredDeclaration>().await;
    let result = runtime.operation(failing::<JitteredDeclaration>(100)).run().await;
    assert!(matches!(result, Err(Error::OperationFailed { .. })));
    let waits = waits(&probe);
    assert_eq!(waits.len(), 4);
    for (wait, (minimum, maximum)) in waits.into_iter().zip([(200, 250), (400, 500), (800, 1000), (1000, 1000)]) {
        assert!((minimum..=maximum).contains(&wait));
    }
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 5);
    runtime.shutdown().await.unwrap();
}

/* === Tests === */
