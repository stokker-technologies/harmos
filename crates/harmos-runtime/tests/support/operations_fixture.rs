/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code, unused_imports)]
/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
pub(crate) use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
pub(crate) use std::time::Duration;
// Third Party Imports
pub(crate) use tokio::sync::Notify;
// Local Imports
use super::support;
pub(crate) use harmos_runtime::operations::{
    Backoff, Context, FailureKind, Jitter, OperationInstanceId, OperationPolicy, RequestShape, RestartPolicy, RetryPolicy,
};
pub(crate) use harmos_runtime::{Error, Refusal, Runtime};
pub(crate) use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

// In-Process Fixture Values Carry Shared Notifications, So They Never Cross A
// Process Boundary: Their Codec Refuses And Encoding Them Is A Defect.
macro_rules! in_process {
    ($($ty:ty),* $(,)?) => {$(
        impl harmos_runtime::Message for $ty {
            const CODEC: &'static str = "fixture.native";
            fn encode(&self) -> Vec<u8> {
                unreachable!("in-process fixture values are never encoded")
            }
            fn encoded_size(&self) -> usize {
                unreachable!("in-process fixture values are never encoded")
            }
            fn decoding_memory(_: &[u8]) -> ::core::result::Result<usize, harmos_runtime::MessageError> {
                Err(harmos_runtime::MessageError::borrowed("in-process fixture values are never decoded"))
            }
            fn decode(_: &[u8]) -> ::core::result::Result<Self, harmos_runtime::MessageError> {
                Err(harmos_runtime::MessageError::borrowed("in-process fixture values are never decoded"))
            }
        }



        impl harmos_runtime::schema::Described for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const FIELDS: &'static [harmos_runtime::schema::Field] = &[];
            const MARKER: &'static [u8] = &harmos_runtime::schema::declaration::encode::<
                { harmos_runtime::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
            >(Self::NAME, Self::FIELDS);
        }
    )*};
}
pub(crate) use in_process;

/* === Type Aliases === */

pub(crate) type Request = Compute;

/* === Structs === */

// Requests Clone Shared Gates; Responses Retain Their Owned Value.

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = Response)]
#[derive(Clone)]
pub(crate) struct Compute {
    pub(crate) value: usize,
    pub(crate) failures: usize,
    pub(crate) failure: FailureKind,
    pub(crate) valid: bool,
    pub(crate) gate: Option<(Arc<Notify>, Arc<Notify>)>,
}

pub(crate) static COMPUTE_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Compute>(
        "test.compute",
        OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: RetryPolicy::OnTransient {
                max_retries: 3,
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: Jitter::None,
                },
            },
        },
    );

pub(crate) const COMPUTE_OPERATION: harmos_runtime::operations::OperationSpec<Compute> =
    harmos_runtime::operations::OperationSpec::<Compute>::new(&COMPUTE_DESCRIPTION);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = Response)]
#[derive(Clone)]
pub(crate) struct Slow(pub(crate) Compute);

pub(crate) static SLOW_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Slow>(
        "test.slow",
        harmos_runtime::OperationPolicy {
            timeout: Duration::from_millis(50),
            attempt_timeout: Duration::from_millis(10),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: COMPUTE_OPERATION.entry().policy.retries,
        },
    );

pub(crate) const SLOW_OPERATION: harmos_runtime::operations::OperationSpec<Slow> =
    harmos_runtime::operations::OperationSpec::<Slow>::new(&SLOW_DESCRIPTION);

#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = Response)]
#[derive(Clone)]
pub(crate) struct Parent(pub(crate) Compute);

pub(crate) static PARENT_DESCRIPTION: harmos_runtime::operations::WireDeclaration =
    harmos_runtime::operations::WireDeclaration::request::<Parent>("test.parent", COMPUTE_OPERATION.entry().policy)
        .visibility(harmos_runtime::operations::Visibility::Public);

static PARENT_CALLEES: [harmos_runtime::operations::WireDeclaration; 1] = [COMPUTE_DESCRIPTION.copied()];
pub(crate) const PARENT_OPERATION: harmos_runtime::operations::OperationSpec<Parent> =
    harmos_runtime::operations::OperationSpec::<Parent>::new(&PARENT_DESCRIPTION).calls(&PARENT_CALLEES);

#[derive(Debug)]
pub(crate) struct Response {
    pub(crate) value: usize,
    pub(crate) attempts: usize,
}

#[derive(Default)]
pub(crate) struct Owner {
    pub(crate) prepared: AtomicUsize,
    pub(crate) runs: AtomicUsize,
    pub(crate) finished: AtomicUsize,
    pub(crate) times: Mutex<Vec<tokio::time::Instant>>,
    pub(crate) seen: Mutex<Vec<OperationInstanceId>>,
    pub(crate) wire_started: Notify,
    pub(crate) wire_joined: AtomicUsize,
    pub(crate) bias: usize,
}

pub(crate) struct ParentOwner;

/* === Enums === */

pub(crate) fn operations_catalog() -> harmos_runtime::operations::Catalog {
    operations_catalog_batch().named(concat!(module_path!(), "::Operations"))
}

fn operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(compute_operation().expect("representable operation"));
    batch = batch.entry(slow_operation().expect("representable operation"));
    batch
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Failed {
    #[error("scripted failure {0:?}")]
    Scripted(FailureKind),

    #[error(transparent)]
    Nested(#[from] Error),

    #[error(transparent)]
    Invocation(#[from] harmos_runtime::operations::InvocationError),
}

/* === Traits === */

/* === Implementations === */

in_process!(Compute, Slow, Parent, Response);

impl harmos_runtime::MemorySize for Compute {
    fn memory_size(&self) -> usize {
        // Shared Notification Resources Have No Charged Fixture Policy.
        if self.gate.is_some() {
            return usize::MAX;
        }
        std::mem::size_of::<Self>()
    }
}

impl harmos_runtime::MemorySize for Slow {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

impl harmos_runtime::MemorySize for Parent {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

impl harmos_runtime::MemorySize for Response {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl Request {
    pub(crate) fn take(&mut self) -> Self {
        Self {
            value: self.value,
            failures: self.failures,
            failure: self.failure,
            valid: self.valid,
            gate: self.gate.take(),
        }
    }
}

struct Attempt<'a>(&'a AtomicUsize);
impl Drop for Attempt<'_> {
    fn drop(&mut self) { self.0.fetch_add(1, Ordering::SeqCst); }
}

impl Owner {
    pub(crate) async fn compute(&self, request: &Request, context: Context<'_>) -> Result<Response, Failed> {
        if !request.valid {
            return Err(Failed::Scripted(FailureKind::Invalid));
        }
        let _attempt = Attempt(&self.finished);
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.times.lock().unwrap().push(context.scope().now());
        self.seen.lock().unwrap().push(context.id());
        let attempts = context.attempt() + 1;
        if request.value == usize::MAX {
            panic!("scripted handler panic");
        }

        if let Some((opened, gate)) = &request.gate {
            opened.notify_one();
            gate.notified().await;
        }
        if attempts <= request.failures {
            return Err(Failed::Scripted(request.failure));
        }
        Ok(Response {
            value: request.value + self.bias,
            attempts,
        })
    }
    pub(crate) fn classify(error: &Failed) -> FailureKind {
        match error {
            Failed::Scripted(failure) => *failure,
            Failed::Nested(_) | Failed::Invocation(_) => FailureKind::Permanent,
        }
    }
}

pub(crate) async fn compute_handler(
    request: Compute, ctx: harmos_runtime::operations::FunctionContext<Compute>,
) -> Result<harmos_runtime::Response<Response>, Failed> {
    let owner = ctx.resource::<Owner>().expect("declared resource is registered");
    let host = ctx.resource::<harmos_runtime::operations::HostContext>().expect("declared resource is registered");
    let context = host.borrow();
    if context.attempt() == 0 {
        owner.prepared.fetch_add(1, Ordering::SeqCst);
    }
    owner.compute(&request, context).await.map(Into::into)
}

fn compute_classification(error: &Failed) -> FailureKind {
    Owner::classify(error)
}

pub(crate) fn compute_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        COMPUTE_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>(), harmos_runtime::Resource::of::<harmos_runtime::operations::HostContext>()] },
        compute_handler,
        compute_classification,
    )
}

pub(crate) async fn slow_handler(
    _request: Slow, ctx: harmos_runtime::operations::FunctionContext<Slow>,
) -> Result<harmos_runtime::Response<Response>, Failed> {
    let owner = ctx.resource::<Owner>().expect("declared resource is registered");
    owner.prepared.fetch_add(1, Ordering::SeqCst);
    owner.runs.fetch_add(1, Ordering::SeqCst);
    std::future::pending().await
}

pub(crate) fn slow_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        SLOW_OPERATION,
        const { &[harmos_runtime::Resource::of::<Owner>()] },
        slow_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

pub(crate) async fn parent_handler(
    request: Parent, ctx: harmos_runtime::operations::FunctionContext<Parent>,
) -> Result<harmos_runtime::Response<Response>, Failed> {
    ctx.operation(COMPUTE_OPERATION, request.0).await.map_err(Failed::Invocation)
}

pub(crate) fn parent_operation()
-> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        PARENT_OPERATION,
        const { &[harmos_runtime::Resource::of::<ParentOwner>()] },
        parent_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

/* === Public Functions === */

/* === Private Functions === */

pub(crate) async fn runtime() -> Runtime<Bank> {
    runtime_with(|_| {}).await
}

pub(crate) async fn runtime_with(update: impl FnOnce(&mut harmos_runtime::Setup<Bank>)) -> Runtime<Bank> {
    crate::support::app::launch::<Bank>(Ledger::default(), (), |setup| {
        setup
            .settings()
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
                ..Default::default()
            })
            .register(catalog());
        setup.resource(Owner::default()).resource(ParentOwner);
        update(setup);
    })
    .await
    .unwrap()
}

#[cfg(feature = "plugin-guest")]
pub(crate) async fn runtime_with_preparation(bytes: std::num::NonZeroUsize) -> Runtime<Bank> {
    runtime_with(|setup| {
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(bytes),
            ..Default::default()
        });
    })
    .await
}

pub(crate) fn request() -> Request {
    Request {
        value: 7,
        failures: 0,
        failure: FailureKind::Transient,
        valid: true,
        gate: None,
    }
}

/* === Tests === */
