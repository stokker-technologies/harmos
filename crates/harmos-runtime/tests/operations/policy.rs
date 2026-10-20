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
use super::{Lanes, runtime};
use harmos_runtime::operations::{
    Backoff, Context, FailureKind, Jitter, Operation, OperationDeclaration, OperationPolicy, ReplaySafety, RestartPolicy, RetryPolicy,
};
use harmos_runtime::{Error, Refusal};

/* === Constants === */

const POLICY_NAME: &str = concat!(module_path!(), "::Policy");
const POLICY_MARKER: &[u8] = &harmos_runtime::schema::declaration::encode::<
    { harmos_runtime::schema::declaration::encoded_len(POLICY_NAME, &[]) },
>(POLICY_NAME, &[]);

/* === Macros === */

// A Policy Request Exposes Its Scripted Behavior To The Shared Handler.
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

        impl Behaves for $ty {
            fn behavior(&mut self) -> &mut Behavior {
                &mut self.behavior
            }
        }
    )*};
}

/* === Type Aliases === */

/* === Structs === */

// Each Request Carries Its Scripted Behavior, Which Can Hold A Shared Gate,
// So Every Policy Request Stays In Process.
struct Policy<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> {
    behavior: Behavior,
}

#[harmos_runtime::operation(
    id = "policy.blocker",
    version = 1,
    response = u64,
    policy = Policy::<30_000, 30_000, 0, 1, false>::POLICY
)]
struct Blocker {
    behavior: Behavior,
}

#[harmos_runtime::operation(
    id = "policy.exponential",
    version = 1,
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
        restart: RestartPolicy::Never,
    }
)]
struct Exponential {
    behavior: Behavior,
}

#[harmos_runtime::operation(
    id = "policy.jittered",
    version = 1,
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
        ..Exponential::POLICY
    }
)]
struct Jittered {
    behavior: Behavior,
}

#[harmos_runtime::operation(
    id = "policy.empty",
    version = 1,
    response = u64,
    policy = OperationPolicy {
        retries: RetryPolicy::OnTransient {
            max_retries: 0,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
        ..Blocker::POLICY
    }
)]
struct EmptyRetries {
    behavior: Behavior,
}

#[harmos_runtime::operation(
    id = "policy.inverted",
    version = 1,
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
        ..Blocker::POLICY
    }
)]
struct InvertedBackoff {
    behavior: Behavior,
}

#[derive(Default)]
struct Probe {
    prepared: AtomicUsize,
    attempts: AtomicUsize,
    dropped: AtomicUsize,
    times: Mutex<Vec<tokio::time::Instant>>,
    started: Notify,
}

struct Owner(Arc<Probe>);

struct State {
    attempt: usize,
    probe: Arc<Probe>,
}

/* === Enums === */

enum Behavior {
    Fail { remaining: usize, kind: std::io::ErrorKind },
    Pending,
    Gate(Arc<Notify>),
}

/* === Traits === */

trait Behaves: OperationDeclaration<Response = u64> + From<Behavior> {
    fn behavior(&mut self) -> &mut Behavior;
}

/* === Implementations === */

impl<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> OperationDeclaration
    for Policy<OVERALL, ATTEMPT, RETRIES, DELAY, REPLAY>
{
    type Response = u64;
    type Input = harmos_runtime::operations::NoStream;
    type Output = harmos_runtime::operations::NoStream;
    const ID: &'static str = "policy.compute";
    const VERSION: u32 = 1;
    const REPLAY: ReplaySafety = if REPLAY { ReplaySafety::Idempotent } else { ReplaySafety::Unknown };
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(OVERALL),
        attempt_timeout: Duration::from_millis(ATTEMPT),
        retries: if RETRIES == 0 {
            RetryPolicy::Never
        } else {
            RetryPolicy::OnTransient {
                max_retries: RETRIES,
                backoff: Backoff::Fixed { delay: Duration::from_millis(DELAY), jitter: Jitter::None },
            }
        },
        restart: RestartPolicy::Never,
    };
}

behaves!(Blocker, Exponential, Jittered, EmptyRetries, InvertedBackoff);

impl<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> harmos_runtime::Message
    for Policy<OVERALL, ATTEMPT, RETRIES, DELAY, REPLAY>
{
    fn encode_to_vec(&self) -> Vec<u8> {
        unreachable!("in-process fixture values are never encoded")
    }
    fn encoded_len(&self) -> usize {
        unreachable!("in-process fixture values are never encoded")
    }
    fn decoding_memory(_: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        Err(harmos_runtime::MessageError::borrowed("in-process fixture values are never decoded"))
    }
    fn decode(_: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        Err(harmos_runtime::MessageError::borrowed("in-process fixture values are never decoded"))
    }
}

impl<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> harmos_runtime::schema::Described
    for Policy<OVERALL, ATTEMPT, RETRIES, DELAY, REPLAY>
{
    const NAME: &'static str = POLICY_NAME;
    const FIELDS: &'static [harmos_runtime::schema::Field] = &[];
    const MARKER: &'static [u8] = POLICY_MARKER;
}

impl<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> harmos_runtime::MemorySize
    for Policy<OVERALL, ATTEMPT, RETRIES, DELAY, REPLAY>
{
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> From<Behavior>
    for Policy<OVERALL, ATTEMPT, RETRIES, DELAY, REPLAY>
{
    fn from(behavior: Behavior) -> Self {
        Self { behavior }
    }
}

impl<const OVERALL: u64, const ATTEMPT: u64, const RETRIES: usize, const DELAY: u64, const REPLAY: bool> Behaves
    for Policy<OVERALL, ATTEMPT, RETRIES, DELAY, REPLAY>
{
    fn behavior(&mut self) -> &mut Behavior {
        &mut self.behavior
    }
}

struct Attempting<O> {
    resources: Arc<Owner>,
    state: State,
    _declaration: std::marker::PhantomData<fn() -> O>,
}

impl<O: Behaves> Operation for Attempting<O> {
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Declaration = O;
    type Resources = Owner;
    type Error = std::io::Error;
    async fn initialize<'ctx>(resources: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        resources.0.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempt: 0, probe: Arc::clone(&resources.0) };
        Ok(Self { resources, state, _declaration: std::marker::PhantomData })
    }
    async fn run<'ctx>(
        &mut self, context: Self::Context<'ctx>, request: &mut O, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let result: ::core::result::Result<u64, Self::Error> = async move {
            self.state.attempt += 1;
            self.resources.0.attempts.fetch_add(1, Ordering::SeqCst);
            self.resources.0.times.lock().unwrap().push(context.scope().now());
            self.resources.0.started.notify_one();
            match request.behavior() {
                Behavior::Fail { remaining, kind } if *remaining > 0 => {
                    *remaining -= 1;
                    return Err(std::io::Error::new(*kind, "retained attempt failure"));
                }
                Behavior::Pending => std::future::pending::<()>().await,
                Behavior::Gate(gate) => gate.notified().await,
                _ => {}
            }
            Ok(u64::try_from(self.state.attempt).expect("attempts fit in u64"))
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    fn classify(error: &Self::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.probe.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn failing<O: Behaves>(remaining: usize) -> O {
    O::from(Behavior::Fail { remaining, kind: std::io::ErrorKind::Interrupted })
}

fn waits(probe: &Probe) -> Vec<u64> {
    probe.times.lock().unwrap().windows(2).map(|pair| u64::try_from((pair[1] - pair[0]).as_millis()).unwrap()).collect()
}

async fn bound<O: Behaves>() -> (harmos_runtime::Runtime<super::support::Bank>, Arc<Probe>) {
    let runtime = runtime().await;
    let probe = Arc::new(Probe::default());
    runtime.operations.register_operation::<Attempting<O>>("policy", Lanes::Serial, Arc::new(Owner(Arc::clone(&probe)))).unwrap();
    (runtime, probe)
}

async fn invalid<O: Behaves>() {
    let runtime = runtime().await;
    let probe = Arc::new(Probe::default());
    let error =
        runtime.operations.register_operation::<Attempting<O>>("policy", Lanes::Serial, Arc::new(Owner(Arc::clone(&probe)))).unwrap_err();
    assert!(matches!(error, Error::OperationDeclaration { .. }));
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 0);
    assert!(runtime.operations.definition(O::ID).is_err());
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn queued_deadline_starts() {
    type Queued = Policy<50, 1000, 0, 1, false>;
    let (runtime, queued_probe) = bound::<Queued>().await;
    let blocker_probe = Arc::new(Probe::default());
    runtime
        .operations
        .register_operation::<Attempting<Blocker>>("policy", Lanes::Serial, Arc::new(Owner(Arc::clone(&blocker_probe))))
        .unwrap();
    let gate = Arc::new(Notify::new());
    let blocker = runtime.operations.submit(Blocker { behavior: Behavior::Gate(Arc::clone(&gate)) }).unwrap();
    blocker_probe.started.notified().await;
    let queued = runtime.operations.submit(failing::<Queued>(0)).unwrap();
    tokio::time::advance(Duration::from_millis(100)).await;
    assert!(matches!(queued.await, Err(Error::OperationRefused(Refusal::TimedOut))));
    // An instance that expires while queued is never constructed.
    assert_eq!(queued_probe.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(queued_probe.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(queued_probe.dropped.load(Ordering::SeqCst), 0);
    gate.notify_one();
    assert_eq!(*blocker.await.unwrap(), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn attempt_limits_backoff() {
    type Attempt = Policy<100, 2000, 0, 1, false>;
    let (runtime, probe) = bound::<Attempt>().await;
    let submitted = tokio::time::Instant::now();
    let result = runtime.operations.call(Attempt { behavior: Behavior::Pending }).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(submitted.elapsed(), Duration::from_millis(100));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();

    type Retrying = Policy<100, 1000, 10, 1000, false>;
    let (runtime, probe) = bound::<Retrying>().await;
    let submitted = tokio::time::Instant::now();
    let result = runtime.operations.call(failing::<Retrying>(100)).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(submitted.elapsed(), Duration::from_millis(100));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn retry_reuses_state() {
    type Retrying = Policy<1000, 100, 2, 20, false>;
    let (runtime, probe) = bound::<Retrying>().await;
    assert_eq!(*runtime.operations.call(failing::<Retrying>(2)).await.unwrap(), 3);
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 3);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    assert_eq!(waits(&probe), [20, 20]);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn reaches_wait_ceiling() {
    let (runtime, probe) = bound::<Exponential>().await;
    let result = runtime.operations.call(failing::<Exponential>(100)).await;
    let Err(Error::OperationFailed { source, .. }) = result else { panic!("the last typed attempt failure is retained") };
    assert_eq!(source.downcast_ref::<std::io::Error>().unwrap().to_string(), "retained attempt failure");
    assert_eq!(waits(&probe), [200, 400, 800, 1000]);
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 5);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn cancellation_skips_backoff() {
    type Retrying = Policy<10000, 100, 2, 1000, false>;
    let (runtime, probe) = bound::<Retrying>().await;
    let invocation = runtime.operations.submit(failing::<Retrying>(100)).unwrap();
    probe.started.notified().await;
    let cancelled = tokio::time::Instant::now();
    invocation.cancel();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(cancelled.elapsed(), Duration::ZERO);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn timeouts_require_replay() {
    type Unknown = Policy<1000, 5, 1, 1, false>;
    let (runtime, probe) = bound::<Unknown>().await;
    let result = runtime.operations.call(Unknown { behavior: Behavior::Pending }).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();

    type Replayable = Policy<1000, 5, 1, 1, true>;
    let (runtime, probe) = bound::<Replayable>().await;
    let result = runtime.operations.call(Replayable { behavior: Behavior::Pending }).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(waits(&probe), [6]);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn policy_refuses_binding() {
    invalid::<Policy<0, 1, 0, 1, false>>().await;
    invalid::<Policy<10, 0, 0, 1, false>>().await;
    invalid::<Policy<10, 5, 1, 0, false>>().await;
    invalid::<EmptyRetries>().await;
    invalid::<InvertedBackoff>().await;
}

#[tokio::test]
async fn calls_settle_retry() {
    type Queued = Policy<1000, 1000, 0, 1, false>;
    let (runtime, probe) = bound::<Queued>().await;
    let gate = Arc::new(Notify::new());
    let first = runtime.operations.submit(Queued { behavior: Behavior::Gate(gate) }).unwrap();
    probe.started.notified().await;
    let second = runtime.operations.submit(failing::<Queued>(0)).unwrap();
    second.cancel();
    assert!(matches!(second.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    first.cancel();
    assert!(matches!(first.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    // The cancelled queued instance was never constructed.
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(probe.dropped.load(Ordering::SeqCst), 1);
    runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn jitter_respects_ceiling() {
    let (runtime, probe) = bound::<Jittered>().await;
    let result = runtime.operations.call(failing::<Jittered>(100)).await;
    assert!(matches!(result, Err(Error::OperationFailed { .. })));
    let waits = waits(&probe);
    assert_eq!(waits.len(), 4);
    for (wait, (minimum, maximum)) in waits.into_iter().zip([(200, 250), (400, 500), (800, 1000), (1000, 1000)]) {
        assert!((minimum..=maximum).contains(&wait));
    }
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 5);
    runtime.stop().await.unwrap();
}

/* === Tests === */
