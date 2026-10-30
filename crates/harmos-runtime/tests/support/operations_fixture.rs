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
    Backoff, CacheKey, Context, FailureKind, Jitter, OperationDeclaration, OperationInstanceId, OperationPolicy, RestartPolicy, RetryPolicy,
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

#[harmos_runtime::operation(codec = manual, id = "test.compute", response = Response,
    resources = [Owner, harmos_runtime::operations::HostContext], key = |request: &Compute| (request.keyed && request.valid).then(|| CacheKey::new(request.value.to_le_bytes().to_vec()).unwrap()),
    policy = OperationPolicy {
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
)]
#[derive(Clone)]
pub(crate) struct Compute {
    pub(crate) value: usize,
    pub(crate) failures: usize,
    pub(crate) failure: FailureKind,
    pub(crate) valid: bool,
    pub(crate) keyed: bool,
    pub(crate) gate: Option<(Arc<Notify>, Arc<Notify>)>,
}

#[harmos_runtime::operation(codec = manual, id = "test.slow", response = Response,
    resources = [Owner],
    policy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_millis(50),
        attempt_timeout: Duration::from_millis(10),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: Compute::POLICY.retries,
    },
)]
#[derive(Clone)]
pub(crate) struct Slow(pub(crate) Compute);

#[harmos_runtime::operation(codec = manual, id = "test.parent", response = Response,
    resources = [ParentOwner], operations = [Compute], visibility = Public,
    policy = Compute::POLICY,
)]
#[derive(Clone)]
pub(crate) struct Parent(pub(crate) Compute);

#[derive(Debug)]
pub(crate) struct Response {
    pub(crate) value: usize,
    pub(crate) attempts: usize,
}

#[derive(Default)]
pub(crate) struct Owner {
    pub(crate) prepared: AtomicUsize,
    pub(crate) runs: AtomicUsize,
    pub(crate) seen: Mutex<Vec<OperationInstanceId>>,
    pub(crate) wire_started: Notify,
    pub(crate) wire_joined: AtomicUsize,
    pub(crate) bias: usize,
}

pub(crate) struct ParentOwner;

/* === Enums === */

#[harmos_runtime::catalog]
pub(crate) enum Operations {
    Compute(Compute),
    Slow(Slow),
}

#[harmos_runtime::catalog]
pub(crate) enum Single<O: harmos_runtime::OperationDeclaration> {
    Operation(O),
}

pub(crate) use support::Lanes;

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
            keyed: self.keyed,
            gate: self.gate.take(),
        }
    }
}

impl Owner {
    pub(crate) async fn compute(&self, request: &Request, context: Context<'_>) -> Result<Response, Failed> {
        if !request.valid {
            return Err(Failed::Scripted(FailureKind::Invalid));
        }
        self.runs.fetch_add(1, Ordering::SeqCst);
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

impl harmos_runtime::Operation for Compute {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Response>, Failed> {
        let owner = ctx.resource::<Owner>();
        let host = ctx.resource::<harmos_runtime::operations::HostContext>();
        let context = host.borrow();
        if context.attempt() == 0 {
            owner.prepared.fetch_add(1, Ordering::SeqCst);
        }
        owner.compute(&self, context).await.map(Into::into)
    }

    fn classify(error: &Failed) -> FailureKind {
        Owner::classify(error)
    }
}

impl harmos_runtime::Operation for Slow {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Response>, Failed> {
        let owner = ctx.resource::<Owner>();
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        owner.runs.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
}

impl harmos_runtime::Operation for Parent {
    type Error = Failed;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<Response>, Failed> {
        ctx.operation(self.0).await.map_err(Failed::Invocation)
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(crate) async fn runtime() -> Runtime<Bank> {
    runtime_with(|_| {}).await
}

pub(crate) async fn runtime_with(update: impl FnOnce(&mut harmos_runtime::Setup<Bank>)) -> Runtime<Bank> {
    Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
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

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
        keyed: false,
        gate: None,
    }
}

/* === Tests === */
