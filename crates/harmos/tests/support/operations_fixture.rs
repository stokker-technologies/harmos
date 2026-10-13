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
pub(crate) use harmos::operations::{
    Backoff, CacheKey, Context, Failure, Jitter, Operation, OperationDeclaration,
    OperationInstanceId, OperationPolicy, RetryPolicy,
};
pub(crate) use harmos::{Error, Refusal, Runtime};
pub(crate) use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Compute;

pub(crate) struct Slow;

pub(crate) struct Parent;

// Owned request/state/response deliberately implement neither Clone nor serde.
pub(crate) struct Request {
    pub(crate) value: usize,
    pub(crate) failures: usize,
    pub(crate) failure: Failure,
    pub(crate) valid: bool,
    pub(crate) keyed: bool,
    pub(crate) gate: Option<(Arc<Notify>, Arc<Notify>)>,
}

pub(crate) struct State {
    pub(crate) attempts: usize,
}

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

pub(crate) struct ComputeCall {
    pub(crate) owner: Arc<Owner>,
    pub(crate) state: State,
}
pub(crate) struct SlowCall {
    pub(crate) owner: Arc<Owner>,
}
pub(crate) struct ParentCall {
    pub(crate) _owner: Arc<ParentOwner>,
}

/* === Enums === */
#[harmos::catalog]
pub(crate) enum Operations {
    Compute(Compute),
    Slow(Slow),
}

#[harmos::catalog]
pub(crate) enum Implemented {
    Compute(ComputeCall),
    Slow(SlowCall),
}

#[harmos::lanes]
pub(crate) enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Serial,
    #[lane(concurrent(2))]
    Concurrent,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Failed {
    #[error("scripted failure {0:?}")]
    Scripted(Failure),
    #[error(transparent)]
    Nested(#[from] Error),
}

/* === Traits === */

/* === Implementations === */
impl harmos::MemorySize for Request {
    fn memory_size(&self) -> usize {
        // Shared Notification Resources Have No Charged Fixture Policy.
        if self.gate.is_some() {
            return usize::MAX;
        }
        std::mem::size_of::<Self>()
    }
}

impl OperationDeclaration for Compute {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "test.compute";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(1),
        retries: RetryPolicy::OnTransient {
            max_retries: 3,
            backoff: Backoff::Fixed {
                delay: Duration::from_millis(1),
                jitter: Jitter::None,
            },
        },
    };
    fn key(request: &Request) -> Option<CacheKey> {
        (request.keyed && request.valid)
            .then(|| CacheKey::new(request.value.to_le_bytes().to_vec()).unwrap())
    }
}

impl OperationDeclaration for Slow {
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "test.slow";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(50),
        attempt_timeout: Duration::from_millis(10),
        retries: Compute::POLICY.retries,
    };
}

impl OperationDeclaration for Parent {
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Shared;
    type Request = Request;
    type Response = Response;
    const ID: &'static str = "test.parent";
    const POLICY: OperationPolicy = Compute::POLICY;
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
    pub(crate) async fn compute(
        &self,
        request: &Request,
        state: &mut State,
        context: Context<'_>,
    ) -> Result<Response, Failed> {
        if !request.valid {
            return Err(Failed::Scripted(Failure::Invalid));
        }
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.seen.lock().unwrap().push(context.id());
        state.attempts += 1;
        if request.value == usize::MAX {
            panic!("scripted handler panic");
        }
        assert_eq!(context.attempt(), state.attempts - 1);
        if let Some((opened, gate)) = &request.gate {
            opened.notify_one();
            gate.notified().await;
        }
        if state.attempts <= request.failures {
            return Err(Failed::Scripted(request.failure));
        }
        Ok(Response {
            value: request.value + self.bias,
            attempts: state.attempts,
        })
    }
    pub(crate) fn classify(error: &Failed) -> Failure {
        match error {
            Failed::Scripted(failure) => *failure,
            Failed::Nested(_) => Failure::Permanent,
        }
    }
}

impl Operation for ComputeCall {
    type Declaration = Compute;
    type Resources = Owner;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = Failed;
    async fn initialize<'ctx>(owner: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        let state = State { attempts: 0 };
        Ok(Self { owner, state })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Request,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Response>, Failed> {
        let result: ::core::result::Result<Response, Self::Error> =
            async move { self.owner.compute(request, &mut self.state, context).await }.await;
        result.map(::core::convert::Into::into)
    }
    fn classify(error: &Failed) -> Failure {
        Owner::classify(error)
    }
}

impl Operation for SlowCall {
    type Declaration = Slow;
    type Resources = Owner;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = Failed;
    async fn initialize<'ctx>(owner: Arc<Owner>, _: Self::Context<'ctx>) -> Result<Self, Failed> {
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { owner })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Request,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Response>, Failed> {
        let result: ::core::result::Result<Response, Self::Error> = async move {
            self.owner.runs.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok(Response {
                value: request.value,
                attempts: 1,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Operation for ParentCall {
    type Declaration = Parent;
    type Resources = ParentOwner;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = Failed;
    async fn initialize<'ctx>(
        owner: Arc<ParentOwner>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Failed> {
        Ok(Self { _owner: owner })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut Request,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Response>, Failed> {
        let result: ::core::result::Result<Response, Self::Error> = async move {
            let response = context.call_as::<Compute>(request.take()).await?;
            Ok(Response {
                value: response.value,
                attempts: response.attempts,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(crate) async fn runtime() -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(16 * 1024).unwrap()),
            ..Default::default()
        })
        .register(catalog())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap()
}

#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
pub(crate) async fn runtime_with_preparation(bytes: std::num::NonZeroUsize) -> Runtime<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(bytes),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap()
}

pub(crate) fn request() -> Request {
    Request {
        value: 7,
        failures: 0,
        failure: Failure::Transient,
        valid: true,
        keyed: false,
        gate: None,
    }
}

/* === Tests === */
