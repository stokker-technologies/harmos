/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

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
use super::runtime_with;
use harmos_runtime::operations::{Backoff, FailureKind, Jitter, OperationPolicy, ReplaySafety, RequestShape, RetryPolicy};
use harmos_runtime::{Error, Refusal};

/* === Macros === */

// Each Request Executes Its Scripted Policy With Declared Resources.

/* === Structs === */

// Each Request Carries Its Scripted Behavior, Which Can Hold a Shared Gate,
// So Every Policy Request Stays In Process.
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]
struct BlockerDeclaration {
    behavior: Behavior,
}

static BLOCKER_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<BlockerDeclaration>("policy.blocker", policy(30_000, 30_000, 0, 1));

const BLOCKER_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<BlockerDeclaration> =
    harmos_runtime::operations::OperationSpec::<BlockerDeclaration>::new(&BLOCKER_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]
struct ExponentialDeclaration {
    behavior: Behavior,
}

static EXPONENTIAL_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<ExponentialDeclaration>(
        "policy.exponential",
        OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 4,
                backoff: Backoff::Exponential {
                    initial: Duration::from_millis(200),
                    maximum: Duration::from_secs(1),
                    jitter: Jitter::None,
                },
            },
        },
    );

const EXPONENTIAL_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<ExponentialDeclaration> =
    harmos_runtime::operations::OperationSpec::<ExponentialDeclaration>::new(&EXPONENTIAL_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]
struct JitteredDeclaration {
    behavior: Behavior,
}

static JITTERED_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<JitteredDeclaration>(
        "policy.jittered",
        OperationPolicy {
            retries: RetryPolicy::OnTransient {
                max_retries: 4,
                backoff: Backoff::Exponential {
                    initial: Duration::from_millis(200),
                    maximum: Duration::from_secs(1),
                    jitter: Jitter::Deterministic,
                },
            },
            ..EXPONENTIAL_DECLARATION_OPERATION.declaration.operation.policy
        },
    );

const JITTERED_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<JitteredDeclaration> =
    harmos_runtime::operations::OperationSpec::<JitteredDeclaration>::new(&JITTERED_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]
struct EmptyRetriesDeclaration {
    behavior: Behavior,
}

static EMPTY_RETRIES_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<EmptyRetriesDeclaration>(
        "policy.empty",
        OperationPolicy {
            retries: RetryPolicy::OnTransient {
                max_retries: 0,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: Jitter::None,
                },
            },
            ..BLOCKER_DECLARATION_OPERATION.declaration.operation.policy
        },
    );

const EMPTY_RETRIES_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<EmptyRetriesDeclaration> =
    harmos_runtime::operations::OperationSpec::<EmptyRetriesDeclaration>::new(&EMPTY_RETRIES_DECLARATION_DESCRIPTION);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]
struct InvertedBackoffDeclaration {
    behavior: Behavior,
}

static INVERTED_BACKOFF_DECLARATION_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<InvertedBackoffDeclaration>(
        "policy.inverted",
        OperationPolicy {
            retries: RetryPolicy::OnTransient {
                max_retries: 1,
                backoff: Backoff::Exponential {
                    initial: Duration::from_millis(100),
                    maximum: Duration::from_millis(50),
                    jitter: Jitter::None,
                },
            },
            ..BLOCKER_DECLARATION_OPERATION.declaration.operation.policy
        },
    );

const INVERTED_BACKOFF_DECLARATION_OPERATION: harmos_runtime::operations::OperationSpec<InvertedBackoffDeclaration> =
    harmos_runtime::operations::OperationSpec::<InvertedBackoffDeclaration>::new(&INVERTED_BACKOFF_DECLARATION_DESCRIPTION);

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
    RequestShape<
        Request = Self,
        Response = u64,
        Input = harmos_runtime::operations::NoStream,
        Output = harmos_runtime::operations::NoStream,
    > + Clone
    + From<Behavior>
    + Sync
{
    fn behavior(&self) -> &Behavior;
}

/* === Implementations === */

super::in_process!(ExponentialDeclaration);

impl harmos_runtime::MemorySize for ExponentialDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl From<Behavior> for ExponentialDeclaration {
    fn from(behavior: Behavior) -> Self {
        Self { behavior }
    }
}

async fn exponential_declaration_handler(
    request: ExponentialDeclaration, ctx: harmos_runtime::operations::FunctionContext<ExponentialDeclaration>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    attempt(request, ctx).await
}

fn exponential_declaration_classification(error: &std::io::Error) -> FailureKind {
    match error.kind() {
        std::io::ErrorKind::Interrupted => FailureKind::Transient,
        std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
        _ => FailureKind::Permanent,
    }
}

fn exponential_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        EXPONENTIAL_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        exponential_declaration_handler,
        exponential_declaration_classification,
    )
}

impl Behaves for ExponentialDeclaration {
    fn behavior(&self) -> &Behavior {
        &self.behavior
    }
}

super::in_process!(JitteredDeclaration);

impl harmos_runtime::MemorySize for JitteredDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl From<Behavior> for JitteredDeclaration {
    fn from(behavior: Behavior) -> Self {
        Self { behavior }
    }
}

async fn jittered_declaration_handler(
    request: JitteredDeclaration, ctx: harmos_runtime::operations::FunctionContext<JitteredDeclaration>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    attempt(request, ctx).await
}

fn jittered_declaration_classification(error: &std::io::Error) -> FailureKind {
    match error.kind() {
        std::io::ErrorKind::Interrupted => FailureKind::Transient,
        std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
        _ => FailureKind::Permanent,
    }
}

fn jittered_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        JITTERED_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        jittered_declaration_handler,
        jittered_declaration_classification,
    )
}

impl Behaves for JitteredDeclaration {
    fn behavior(&self) -> &Behavior {
        &self.behavior
    }
}

super::in_process!(EmptyRetriesDeclaration);

impl harmos_runtime::MemorySize for EmptyRetriesDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl From<Behavior> for EmptyRetriesDeclaration {
    fn from(behavior: Behavior) -> Self {
        Self { behavior }
    }
}

async fn empty_retries_declaration_handler(
    request: EmptyRetriesDeclaration, ctx: harmos_runtime::operations::FunctionContext<EmptyRetriesDeclaration>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    attempt(request, ctx).await
}

fn empty_retries_declaration_classification(error: &std::io::Error) -> FailureKind {
    match error.kind() {
        std::io::ErrorKind::Interrupted => FailureKind::Transient,
        std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
        _ => FailureKind::Permanent,
    }
}

fn empty_retries_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        EMPTY_RETRIES_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        empty_retries_declaration_handler,
        empty_retries_declaration_classification,
    )
}

impl Behaves for EmptyRetriesDeclaration {
    fn behavior(&self) -> &Behavior {
        &self.behavior
    }
}

super::in_process!(InvertedBackoffDeclaration);

impl harmos_runtime::MemorySize for InvertedBackoffDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl From<Behavior> for InvertedBackoffDeclaration {
    fn from(behavior: Behavior) -> Self {
        Self { behavior }
    }
}

async fn inverted_backoff_declaration_handler(
    request: InvertedBackoffDeclaration, ctx: harmos_runtime::operations::FunctionContext<InvertedBackoffDeclaration>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    attempt(request, ctx).await
}

fn inverted_backoff_declaration_classification(error: &std::io::Error) -> FailureKind {
    match error.kind() {
        std::io::ErrorKind::Interrupted => FailureKind::Transient,
        std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
        _ => FailureKind::Permanent,
    }
}

fn inverted_backoff_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        INVERTED_BACKOFF_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        inverted_backoff_declaration_handler,
        inverted_backoff_declaration_classification,
    )
}

impl Behaves for InvertedBackoffDeclaration {
    fn behavior(&self) -> &Behavior {
        &self.behavior
    }
}

super::in_process!(BlockerDeclaration);

impl harmos_runtime::MemorySize for BlockerDeclaration {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

async fn blocker_declaration_handler(
    request: BlockerDeclaration, context: harmos_runtime::operations::FunctionContext<BlockerDeclaration>,
) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
    let probe = context.resource::<BlockerOwner>().expect("declared resource is registered");
    probe.0.attempts.fetch_add(1, Ordering::SeqCst);
    probe.0.started.notify_one();
    let Behavior::Gate(gate) = request.behavior else { unreachable!("the blocker owns a release gate") };
    gate.notified().await;
    Ok(1.into())
}

fn blocker_declaration_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        BLOCKER_DECLARATION_OPERATION,
        const { &[harmos_runtime::Resource::of::<BlockerOwner>()] },
        blocker_declaration_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Private Functions === */

const fn policy(overall: u64, attempt: u64, retries: usize, delay: u64) -> OperationPolicy {
    OperationPolicy {
        timeout: Duration::from_millis(overall),
        attempt_timeout: Duration::from_millis(attempt),
        cancellation_timeout: std::time::Duration::ZERO,
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

async fn attempt<O: Behaves>(
    request: O, ctx: harmos_runtime::operations::FunctionContext<O>,
) -> std::io::Result<harmos_runtime::Response<u64>> {
    let resources = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
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

async fn bound(operation: harmos_runtime::operations::OperationFactory) -> (harmos_runtime::Runtime<super::support::Bank>, Arc<Probe>) {
    bound_with(operation, |_| {}).await
}

async fn bound_with(
    operation: harmos_runtime::operations::OperationFactory, update: impl FnOnce(&mut harmos_runtime::Setup<super::support::Bank>),
) -> (harmos_runtime::Runtime<super::support::Bank>, Arc<Probe>) {
    let probe = Arc::new(Probe::default());
    let runtime = runtime_with(|setup| {
        setup.resource(Owner(probe.clone()));
        update(setup);
    })
    .await;
    runtime.operations.register_operation(operation, crate::support::SERIAL).await.unwrap();
    (runtime, probe)
}

async fn invalid(operation: harmos_runtime::operations::OperationFactory) {
    let id = operation.definition.id.to_string();
    let probe = Arc::new(Probe::default());
    let runtime = runtime_with(|setup| {
        setup.resource(Owner(probe.clone()));
    })
    .await;
    let error = runtime.operations.register_operation(operation, crate::support::SERIAL).await.unwrap_err();
    assert!(matches!(error, Error::OperationDeclaration { .. }));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 0);
    assert!(runtime.operations.definition(&id).is_err());
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn queued_deadline_starts() {
    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Queued {
        behavior: Behavior,
    }

    static QUEUED_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<Queued>("policy.compute", policy(50, 1000, 0, 1)).replay(if false {
            ReplaySafety::Idempotent
        } else {
            ReplaySafety::Unknown
        });

    const QUEUED_OPERATION: harmos_runtime::operations::OperationSpec<Queued> =
        harmos_runtime::operations::OperationSpec::<Queued>::new(&QUEUED_DESCRIPTION);

    super::in_process!(Queued);

    impl harmos_runtime::MemorySize for Queued {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Queued {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn queued_handler(
        request: Queued, ctx: harmos_runtime::operations::FunctionContext<Queued>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn queued_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn queued_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            QUEUED_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            queued_handler,
            queued_classification,
        )
    }

    impl Behaves for Queued {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let blocker_probe = Arc::new(Probe::default());
    let (runtime, queued_probe) = bound_with(queued_operation().expect("representable operation"), |setup| {
        setup.resource(BlockerOwner(blocker_probe.clone()));
    })
    .await;
    runtime.operations.register_operation(blocker_declaration_operation().unwrap(), crate::support::SERIAL).await.unwrap();
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
    assert!(
        matches!(queued.await, Err(Error::OperationRefused(Refusal::TimedOut { operation, attempt: 1, timeout, configured_by: "operation policy.timeout" })) if operation == QUEUED_OPERATION.declaration.operation.id && timeout == Duration::from_millis(50))
    );
    // An Expired Queued Request Never Starts An Attempt.
    assert_eq!(queued_probe.attempts.load(Ordering::SeqCst), 0);
    gate.notify_one();
    assert_eq!(*blocker.await.unwrap(), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn attempt_limits_backoff() {
    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Attempt {
        behavior: Behavior,
    }

    static ATTEMPT_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<Attempt>("policy.compute", policy(100, 2000, 0, 1)).replay(if false {
            ReplaySafety::Idempotent
        } else {
            ReplaySafety::Unknown
        });

    const ATTEMPT_OPERATION: harmos_runtime::operations::OperationSpec<Attempt> =
        harmos_runtime::operations::OperationSpec::<Attempt>::new(&ATTEMPT_DESCRIPTION);

    super::in_process!(Attempt);

    impl harmos_runtime::MemorySize for Attempt {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Attempt {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn attempt_handler(
        request: Attempt, ctx: harmos_runtime::operations::FunctionContext<Attempt>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn attempt_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn attempt_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            ATTEMPT_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            attempt_handler,
            attempt_classification,
        )
    }

    impl Behaves for Attempt {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let (runtime, probe) = bound(attempt_operation().expect("representable operation")).await;
    let submitted = tokio::time::Instant::now();
    let result = runtime.operation(Attempt { behavior: Behavior::Pending }).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut { .. }))));
    assert_eq!(submitted.elapsed(), Duration::from_millis(100));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();

    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Retrying {
        behavior: Behavior,
    }

    static RETRYING_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<
        Retrying,
    >("policy.compute", policy(100, 1000, 10, 1000))
    .replay(if false { ReplaySafety::Idempotent } else { ReplaySafety::Unknown });

    const RETRYING_OPERATION: harmos_runtime::operations::OperationSpec<Retrying> =
        harmos_runtime::operations::OperationSpec::<Retrying>::new(&RETRYING_DESCRIPTION);

    super::in_process!(Retrying);

    impl harmos_runtime::MemorySize for Retrying {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Retrying {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn retrying_handler(
        request: Retrying, ctx: harmos_runtime::operations::FunctionContext<Retrying>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn retrying_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn retrying_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            RETRYING_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            retrying_handler,
            retrying_classification,
        )
    }

    impl Behaves for Retrying {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let (runtime, probe) = bound(retrying_operation().expect("representable operation")).await;
    let submitted = tokio::time::Instant::now();
    let result = runtime.operation(failing::<Retrying>(100)).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut { .. }))));
    assert_eq!(submitted.elapsed(), Duration::from_millis(100));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn reaches_wait_ceiling() {
    let (runtime, probe) = bound(exponential_declaration_operation().expect("representable operation")).await;
    let result = runtime.operation(failing::<ExponentialDeclaration>(100)).await;
    let Err(Error::OperationFailed { source, .. }) = result else { panic!("the last typed attempt failure is retained") };
    assert_eq!(source.downcast_ref::<std::io::Error>().unwrap().kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(waits(&probe), [200, 400, 800, 1000]);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 5);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn cancellation_skips_backoff() {
    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Retrying {
        behavior: Behavior,
    }

    static RETRYING_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<
        Retrying,
    >("policy.compute", policy(10000, 100, 2, 1000))
    .replay(if false { ReplaySafety::Idempotent } else { ReplaySafety::Unknown });

    const RETRYING_OPERATION: harmos_runtime::operations::OperationSpec<Retrying> =
        harmos_runtime::operations::OperationSpec::<Retrying>::new(&RETRYING_DESCRIPTION);

    super::in_process!(Retrying);

    impl harmos_runtime::MemorySize for Retrying {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Retrying {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn retrying_handler(
        request: Retrying, ctx: harmos_runtime::operations::FunctionContext<Retrying>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn retrying_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn retrying_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            RETRYING_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            retrying_handler,
            retrying_classification,
        )
    }

    impl Behaves for Retrying {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let (runtime, probe) = bound(retrying_operation().expect("representable operation")).await;
    let invocation = runtime.operation(failing::<Retrying>(100)).start().await.unwrap();
    probe.started.notified().await;
    let cancelled = tokio::time::Instant::now();
    invocation.request_cancel();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(cancelled.elapsed(), Duration::ZERO);
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn timeouts_require_replay() {
    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Unknown {
        behavior: Behavior,
    }

    static UNKNOWN_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<Unknown>("policy.compute", policy(1000, 5, 1, 1)).replay(if false {
            ReplaySafety::Idempotent
        } else {
            ReplaySafety::Unknown
        });

    const UNKNOWN_OPERATION: harmos_runtime::operations::OperationSpec<Unknown> =
        harmos_runtime::operations::OperationSpec::<Unknown>::new(&UNKNOWN_DESCRIPTION);

    super::in_process!(Unknown);

    impl harmos_runtime::MemorySize for Unknown {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Unknown {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn unknown_handler(
        request: Unknown, ctx: harmos_runtime::operations::FunctionContext<Unknown>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn unknown_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn unknown_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            UNKNOWN_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            unknown_handler,
            unknown_classification,
        )
    }

    impl Behaves for Unknown {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let (runtime, probe) = bound(unknown_operation().expect("representable operation")).await;
    let result = runtime.operation(Unknown { behavior: Behavior::Pending }).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut { .. }))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();

    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Replayable {
        behavior: Behavior,
    }

    static REPLAYABLE_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<
        Replayable,
    >("policy.compute", policy(1000, 5, 1, 1))
    .replay(if true { ReplaySafety::Idempotent } else { ReplaySafety::Unknown });

    const REPLAYABLE_OPERATION: harmos_runtime::operations::OperationSpec<Replayable> =
        harmos_runtime::operations::OperationSpec::<Replayable>::new(&REPLAYABLE_DESCRIPTION);

    super::in_process!(Replayable);

    impl harmos_runtime::MemorySize for Replayable {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Replayable {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn replayable_handler(
        request: Replayable, ctx: harmos_runtime::operations::FunctionContext<Replayable>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn replayable_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn replayable_operation()
    -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
        harmos_runtime::operations::OperationFactory::function(
            REPLAYABLE_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            replayable_handler,
            replayable_classification,
        )
    }

    impl Behaves for Replayable {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let (runtime, probe) = bound(replayable_operation().expect("representable operation")).await;
    let result = runtime.operation(Replayable { behavior: Behavior::Pending }).await;
    assert!(matches!(result, Err(Error::OperationRefused(Refusal::TimedOut { .. }))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(waits(&probe), [6]);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test]
async fn policy_refuses_binding() {
    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Invalid0 {
        behavior: Behavior,
    }

    static INVALID0_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<
        Invalid0,
    >("policy.compute", policy(0, 1, 0, 1))
    .replay(if false { ReplaySafety::Idempotent } else { ReplaySafety::Unknown });

    const INVALID0_OPERATION: harmos_runtime::operations::OperationSpec<Invalid0> =
        harmos_runtime::operations::OperationSpec::<Invalid0>::new(&INVALID0_DESCRIPTION);

    super::in_process!(Invalid0);

    impl harmos_runtime::MemorySize for Invalid0 {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Invalid0 {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn invalid0_handler(
        request: Invalid0, ctx: harmos_runtime::operations::FunctionContext<Invalid0>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn invalid0_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn invalid0_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            INVALID0_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            invalid0_handler,
            invalid0_classification,
        )
    }

    impl Behaves for Invalid0 {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    invalid(invalid0_operation().expect("representable operation")).await;

    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Invalid1 {
        behavior: Behavior,
    }

    static INVALID1_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<
        Invalid1,
    >("policy.compute", policy(10, 0, 0, 1))
    .replay(if false { ReplaySafety::Idempotent } else { ReplaySafety::Unknown });

    const INVALID1_OPERATION: harmos_runtime::operations::OperationSpec<Invalid1> =
        harmos_runtime::operations::OperationSpec::<Invalid1>::new(&INVALID1_DESCRIPTION);

    super::in_process!(Invalid1);

    impl harmos_runtime::MemorySize for Invalid1 {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Invalid1 {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn invalid1_handler(
        request: Invalid1, ctx: harmos_runtime::operations::FunctionContext<Invalid1>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn invalid1_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn invalid1_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            INVALID1_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            invalid1_handler,
            invalid1_classification,
        )
    }

    impl Behaves for Invalid1 {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    invalid(invalid1_operation().expect("representable operation")).await;

    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Invalid2 {
        behavior: Behavior,
    }

    static INVALID2_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<
        Invalid2,
    >("policy.compute", policy(10, 5, 1, 0))
    .replay(if false { ReplaySafety::Idempotent } else { ReplaySafety::Unknown });

    const INVALID2_OPERATION: harmos_runtime::operations::OperationSpec<Invalid2> =
        harmos_runtime::operations::OperationSpec::<Invalid2>::new(&INVALID2_DESCRIPTION);

    super::in_process!(Invalid2);

    impl harmos_runtime::MemorySize for Invalid2 {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Invalid2 {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn invalid2_handler(
        request: Invalid2, ctx: harmos_runtime::operations::FunctionContext<Invalid2>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn invalid2_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn invalid2_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            INVALID2_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            invalid2_handler,
            invalid2_classification,
        )
    }

    impl Behaves for Invalid2 {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    invalid(invalid2_operation().expect("representable operation")).await;
    invalid(empty_retries_declaration_operation().expect("representable operation")).await;
    invalid(inverted_backoff_declaration_operation().expect("representable operation")).await;
}

#[tokio::test]
async fn calls_settle_retry() {
    #[derive(harmos_runtime::Request)]
    #[harmos_path(harmos_runtime)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Queued {
        behavior: Behavior,
    }

    static QUEUED_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
        harmos_runtime::operations::WireDeclaration::request::<Queued>("policy.compute", policy(1000, 1000, 0, 1)).replay(if false {
            ReplaySafety::Idempotent
        } else {
            ReplaySafety::Unknown
        });

    const QUEUED_OPERATION: harmos_runtime::operations::OperationSpec<Queued> =
        harmos_runtime::operations::OperationSpec::<Queued>::new(&QUEUED_DESCRIPTION);

    super::in_process!(Queued);

    impl harmos_runtime::MemorySize for Queued {
        fn memory_size(&self) -> usize {
            std::mem::size_of::<Self>()
        }
    }

    impl From<Behavior> for Queued {
        fn from(behavior: Behavior) -> Self {
            Self { behavior }
        }
    }

    async fn queued_handler(
        request: Queued, ctx: harmos_runtime::operations::FunctionContext<Queued>,
    ) -> Result<harmos_runtime::Response<u64>, std::io::Error> {
        attempt(request, ctx).await
    }

    fn queued_classification(error: &std::io::Error) -> FailureKind {
        match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Transient,
            std::io::ErrorKind::PermissionDenied => FailureKind::Unauthorized,
            _ => FailureKind::Permanent,
        }
    }

    fn queued_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError>
    {
        harmos_runtime::operations::OperationFactory::function(
            QUEUED_OPERATION,
            const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
            queued_handler,
            queued_classification,
        )
    }

    impl Behaves for Queued {
        fn behavior(&self) -> &Behavior {
            &self.behavior
        }
    }

    let (runtime, probe) = bound(queued_operation().expect("representable operation")).await;
    let gate = Arc::new(Notify::new());
    let first = runtime.operation(Queued { behavior: Behavior::Gate(gate) }).start().await.unwrap();
    probe.started.notified().await;
    let second = runtime.operation(failing::<Queued>(0)).start().await.unwrap();
    second.request_cancel();
    assert!(matches!(second.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    first.request_cancel();
    assert!(matches!(first.await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
    // A Cancelled Queued Request Never Starts An Attempt.
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 1);
    crate::support::app::shutdown(&runtime).await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn jitter_respects_ceiling() {
    let (runtime, probe) = bound(jittered_declaration_operation().expect("representable operation")).await;
    let result = runtime.operation(failing::<JitteredDeclaration>(100)).await;
    assert!(matches!(result, Err(Error::OperationFailed { .. })));
    let waits = waits(&probe);
    assert_eq!(waits.len(), 4);
    for (wait, (minimum, maximum)) in waits.into_iter().zip([(200, 250), (400, 500), (800, 1000), (1000, 1000)]) {
        assert!((minimum..=maximum).contains(&wait));
    }
    assert_eq!(probe.attempts.load(Ordering::SeqCst), 5);
    crate::support::app::shutdown(&runtime).await.unwrap();
}
