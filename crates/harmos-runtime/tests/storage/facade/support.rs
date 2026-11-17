/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Facade Policy and Payload Proof

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::error;
use std::future;
use std::io;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use harmos_runtime::operations::{
    ExecutionContext, FailureKind, HostContext, Lane, NativeInvocation, NativeOperation, OperationFactory, Registration, ReplaySafety,
    Resource, ServingError,
};
use harmos_runtime::state::Never;
use harmos_runtime::{
    Application, Backoff, Jitter, Lifecycle, MemorySize, Message, MessageError, OperationPolicy, RetryPolicy, Setup, Startup,
};

/* === Constants === */

pub(super) const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(2),
    attempt_timeout: Duration::from_secs(2),
    cancellation_timeout: Duration::ZERO,
    retries: RetryPolicy::OnTransient {
        total_attempts: 2,
        backoff: Backoff::Fixed {
            delay: Duration::from_millis(1),
            jitter: Jitter::None,
        },
    },
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Model;
pub(super) struct Probe;
pub(super) struct Tag(pub(super) u64);
pub(super) struct Parent;
pub(super) struct Waiting;
pub(super) struct Capture;
pub(super) struct Forward;

#[derive(Default)]
pub(super) struct Gate {
    pub(super) started: Notify,
    pub(super) loaded: Notify,
    pub(super) closed: AtomicUsize,
    pub(super) child: Mutex<Option<NativeInvocation<Waiting>>>,
}

#[derive(Clone)]
pub(super) struct ChildRequest(pub(super) Arc<Gate>);

pub(super) struct Guard(Arc<Gate>);

#[derive(Clone)]
pub(super) struct Payload {
    pub(super) bytes: Arc<[u8]>,
    pub(super) attempts: Arc<AtomicUsize>,
    pub(super) encodes: Arc<AtomicUsize>,
    pub(super) memory_reads: Arc<AtomicUsize>,
    pub(super) fail_until: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lifecycle for Model {
    type Config = ();
    type Error = Infallible;
    type Context = Startup<Self>;

    async fn prepare(_: (), setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
        let factory = OperationFactory::native::<Probe>().unwrap();
        let parent = OperationFactory::native::<Parent>().unwrap();
        let waiting = OperationFactory::native::<Waiting>().unwrap();
        let capture = OperationFactory::native::<Capture>().unwrap();
        let forward = OperationFactory::native::<Forward>().unwrap();
        setup.settings().with_registration(
            Registration::new(Lane::default()).entry(factory).entry(parent).entry(waiting).entry(capture).entry(forward),
        );
        Ok(Self)
    }
}

impl Application for Model {
    type State = ();
    type Transaction = Never;
    type Record = ();
}

impl MemorySize for Payload {
    fn memory_size(&self) -> usize {
        self.memory_reads.fetch_add(1, Ordering::SeqCst);
        mem::size_of::<Self>().saturating_add(self.bytes.len())
    }
}

impl Message for Payload {
    const CODEC: &'static str = "facade.test";

    fn encode(&self) -> Vec<u8> {
        self.encodes.fetch_add(1, Ordering::SeqCst);
        self.bytes.to_vec()
    }

    fn encoded_size(&self) -> usize {
        self.encodes.fetch_add(1, Ordering::SeqCst);
        self.bytes.len()
    }

    fn decode(_: &[u8]) -> Result<Self, MessageError> {
        Err(MessageError::borrowed("Use the Native Payload"))
    }

    fn decoding_memory(_: &[u8]) -> Result<usize, MessageError> {
        Err(MessageError::borrowed("Use the Native Payload"))
    }
}

impl NativeOperation for Probe {
    type Request = Payload;
    type Response = (Arc<[u8]>, Option<u64>);
    type Endpoints = ();
    const ID: &'static str = "facade.probe";
    const POLICY: OperationPolicy = POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;

    fn classify(_: &(dyn error::Error + Send + Sync + 'static)) -> FailureKind {
        FailureKind::Transient
    }

    async fn run(context: ExecutionContext, request: Payload, _: &()) -> Result<Self::Response, ServingError> {
        let attempt = request.attempts.fetch_add(1, Ordering::SeqCst);
        if attempt < request.fail_until {
            return Err(ServingError::Native(Arc::new(io::Error::other("Try the Probe Again"))));
        }
        Ok((request.bytes, context.call.get::<Tag>().map(|tag| tag.0)))
    }
}

impl MemorySize for ChildRequest {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.closed.fetch_add(1, Ordering::SeqCst);
    }
}

impl NativeOperation for Parent {
    type Request = ChildRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "facade.parent";
    const POLICY: OperationPolicy = OperationPolicy { retries: RetryPolicy::Never, ..POLICY };
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];

    async fn run(context: ExecutionContext, request: ChildRequest, _: &()) -> Result<(), ServingError> {
        let host = context.resource::<HostContext>().map_err(|_| ServingError::Catalog)?;
        let child = host.for_call().submit::<Waiting>(request.clone(), ()).map_err(|error| ServingError::Native(Arc::new(error)))?;
        *request.0.child.lock().unwrap() = Some(child);
        request.0.loaded.notify_one();
        context.cancellation.cancelled().await;
        Ok(())
    }
}

impl NativeOperation for Waiting {
    type Request = ChildRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "facade.waiting";
    const POLICY: OperationPolicy = OperationPolicy { retries: RetryPolicy::Never, ..POLICY };

    async fn run(_: ExecutionContext, request: ChildRequest, _: &()) -> Result<(), ServingError> {
        let _guard = Guard(request.0.clone());
        request.0.started.notify_one();
        future::pending().await
    }
}

impl NativeOperation for Capture {
    type Request = ();
    type Response = HostContext;
    type Endpoints = ();
    const ID: &'static str = "facade.capture";
    const POLICY: OperationPolicy = OperationPolicy { retries: RetryPolicy::Never, ..POLICY };
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];

    async fn run(context: ExecutionContext, _: (), _: &()) -> Result<HostContext, ServingError> {
        context.resource::<HostContext>().map(|host| host.as_ref().clone()).map_err(|_| ServingError::Catalog)
    }
}

impl NativeOperation for Forward {
    type Request = Payload;
    type Response = (Arc<[u8]>, Option<u64>);
    type Endpoints = ();
    const ID: &'static str = "facade.forward";
    const POLICY: OperationPolicy = POLICY;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];

    async fn run(context: ExecutionContext, request: Payload, _: &()) -> Result<Self::Response, ServingError> {
        let host = context.resource::<HostContext>().map_err(|_| ServingError::Catalog)?;
        host.for_call().call::<Probe>(request).await.map_err(|error| ServingError::Native(Arc::new(error)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn payload(fail_until: usize) -> Payload {
    Payload {
        bytes: Arc::from([4, 7, 9]),
        attempts: Arc::new(AtomicUsize::new(0)),
        encodes: Arc::new(AtomicUsize::new(0)),
        memory_reads: Arc::new(AtomicUsize::new(0)),
        fail_until,
    }
}
