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
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};
use std::time::Duration;
// Third Party Imports
use tokio::sync::OwnedSemaphorePermit;
// Local Imports
use super::caller::Source;
use super::manager::{Entry, Submission};
use super::{Manager, OperationInstanceId};
use crate::operations::OperationDeclaration;
#[cfg(test)]
use crate::supervision::executor;
use crate::{Error, Refusal, Result, operations::Scope};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Outcome = Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send>>;
pub(super) type Dispatch = fn(&Manager, Submission, Source, Option<Parent<'_>>, EncodedInput) -> Result<EncodedInvocation>;

/* === Structs === */

struct Encoded;

#[derive(Default)]
struct EncodedOptions {
    expected: Option<([u8; 32], u64)>,
    retry: Option<super::RetryPolicy>,
}

pub(super) struct EncodedInput {
    pub(super) payload: Vec<u8>,
    pub(super) retry: Option<super::RetryPolicy>,
    pub(super) preparation: Option<OwnedSemaphorePermit>,
}

#[cfg(feature = "plugin-guest")]
#[derive(Clone)]
pub struct PluginCallback {
    operations: Manager,
    entry: Arc<Entry>,
    contract: crate::operations::Contract,
}

pub(super) struct Parent<'a> {
    pub(super) scope: &'a Scope<'a>,
    pub(super) ancestors: &'a [&'static str],
}

pub struct EncodedInvocation {
    pub(super) id: OperationInstanceId,
    pub(super) generation: super::OperationGeneration,
    pub(super) outcome: Outcome,
    pub(crate) control: Arc<crate::supervision::instance::Instance>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OperationDeclaration for Encoded {
    type Request = Vec<u8>;
    type Response = Vec<u8>;
    type Input = super::NoStream;
    type Output = super::NoStream;
    const ID: &'static str = "encoded";
    const POLICY: super::OperationPolicy = super::OperationPolicy {
        timeout: harmos_domain::operations::UNBOUNDED,
        attempt_timeout: harmos_domain::operations::UNBOUNDED,
        cancellation_timeout: std::time::Duration::ZERO,
        retries: super::RetryPolicy::Never,
    };
}

impl EncodedInvocation {
    pub fn id(&self) -> OperationInstanceId {
        self.id
    }

    pub fn generation(&self) -> super::OperationGeneration {
        self.generation
    }

    pub fn request_cancel(&self) {
        self.control.request_cancel();
    }

    pub async fn cancel(self) -> Result<Vec<u8>> {
        self.request_cancel();
        self.await
    }

    pub fn status(&self) -> super::OperationStatus {
        self.control.status()
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.join", level = "debug", skip_all)]
    pub async fn join(self) -> Result<Vec<u8>> {
        self.await
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.settle", level = "debug", skip_all)]
    pub async fn settle(&self) -> Result<()> {
        self.control.settle().await.map_err(|source| Error::OperationSettlement { source })
    }
}

impl Future for EncodedInvocation {
    type Output = Result<Vec<u8>>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().outcome.as_mut().poll(context)
    }
}

impl Manager {
    pub fn submit_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<EncodedInvocation> {
        self.submit_encoded_as(operation, payload, Source::Host, None)
    }

    pub fn submit_encoded_with(&self, operation: &str, payload: Vec<u8>, retry: Option<super::RetryPolicy>) -> Result<EncodedInvocation> {
        self.submit_encoded_at_as(operation, payload, Source::Host, None, self.clock().now(), EncodedOptions { expected: None, retry })
    }

    pub async fn call_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.submit_encoded(operation, payload)?.await
    }

    pub(super) fn submit_encoded_as(
        &self, operation: &str, payload: Vec<u8>, source: Source, parent: Option<Parent<'_>>,
    ) -> Result<EncodedInvocation> {
        self.submit_encoded_at_as(operation, payload, source, parent, self.clock().now(), EncodedOptions::default())
    }

    #[cfg(feature = "plugin-guest")]
    pub(super) fn submit_encoded_retry_as(
        &self, operation: &str, payload: Vec<u8>, source: Source, retry: Option<super::RetryPolicy>,
    ) -> Result<EncodedInvocation> {
        self.submit_encoded_at_as(operation, payload, source, None, self.clock().now(), EncodedOptions { expected: None, retry })
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.submit_encoded_at_as", level = "debug", skip_all)]
    fn submit_encoded_at_as(
        &self, operation: &str, payload: Vec<u8>, source: Source, parent: Option<Parent<'_>>, submitted: tokio::time::Instant,
        options: EncodedOptions,
    ) -> Result<EncodedInvocation> {
        let origin = source.check()?;
        if let Ok(entry) = self.capture_entry(operation)
            && entry.binding.is::<crate::plugin::grpc::invoke::Binding>()
        {
            return self.dispatch_encoded(entry, payload, source, parent, submitted, options);
        }
        let qualified = crate::QualifiedId::parse(operation).map_err(|_| Error::InvalidInput {
            field: "encoded operation",
            reason: "routing requires an owner-qualified operation identity",
        })?;
        let (owner, local) = qualified.as_str().split_once('.').expect("validated qualified identity");
        let entry = self.capture_routed(local, owner, &origin).map_err(|error| match error {
            Error::NotRegistered { .. } => Error::NotRegistered {
                domain: "operation",
                name: operation.to_owned(),
            },
            error => error,
        })?;
        self.dispatch_encoded(entry, payload, source, parent, submitted, options)
    }

    fn dispatch_encoded(
        &self, entry: Arc<Entry>, payload: Vec<u8>, source: Source, parent: Option<Parent<'_>>, submitted: tokio::time::Instant,
        options: EncodedOptions,
    ) -> Result<EncodedInvocation> {
        let EncodedOptions { expected, retry } = options;
        let origin = source.check()?;
        entry.available_to(&origin)?;
        entry.authorize(&origin)?;
        if expected.is_some_and(|(fingerprint, generation)| {
            entry.declared().fingerprint.0 != fingerprint || entry.generation.value() != generation
        }) {
            return Err(Error::InvalidInput {
                field: "encoded operation identity",
                reason: "fingerprint or generation differs from the assigned target",
            });
        }
        let dispatch = entry.wire.ok_or(Error::InvalidInput {
            field: "encoded operation",
            reason: "operation has no cross-boundary codec binding",
        })?;

        dispatch(
            self,
            Submission {
                submitted,
                entry,
                cause: None,
                preparation: None,
            },
            source,
            parent,
            EncodedInput { retry, payload, preparation: None },
        )
    }
}

#[cfg(feature = "plugin-guest")]
impl Manager {
    pub fn capture_callback(
        &self, scope: &crate::guest::host::plugin::PluginScope, contract: &crate::operations::Contract,
    ) -> Result<PluginCallback> {
        if scope.is_closed() {
            return Err(Error::Closing { domain: "callback scope" });
        }
        let entry = self.capture_provider(contract.operation().id.as_str(), Some(scope.owner()))?;
        let selected = entry.plugin().ok_or(Error::InvalidInput {
            field: "callback scope",
            reason: "callback target has no original plugin generation",
        })?;
        if !selected.same_activation(scope) {
            return Err(Error::InvalidInput {
                field: "callback scope",
                reason: "callback target belongs to a different plugin generation",
            });
        }
        PluginCallback::from_entry(self.clone(), entry, contract)
    }
}

#[cfg(feature = "plugin-guest")]
impl PluginCallback {
    pub(super) fn from_entry(operations: Manager, entry: Arc<Entry>, contract: &crate::operations::Contract) -> Result<Self> {
        if entry.declared() != contract.operation() || entry.contract.as_ref() != Some(contract) {
            return Err(Error::InvalidInput {
                field: "callback contract",
                reason: "callback requires the exact retained canonical contract",
            });
        }
        if entry.wire.is_none() {
            return Err(Error::InvalidInput {
                field: "callback codec",
                reason: "callback target has no original codec binding",
            });
        }
        Ok(Self {
            operations,
            entry,
            contract: contract.clone(),
        })
    }

    pub fn contract(&self) -> &crate::operations::Contract {
        &self.contract
    }

    pub fn submit_with(
        &self, encoding_memory_bytes: std::num::NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>,
    ) -> Result<EncodedInvocation> {
        self.submit_with_retry(encoding_memory_bytes, encode, None)
    }

    pub fn submit_with_retry(
        &self, encoding_memory_bytes: std::num::NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>,
        retry: Option<super::RetryPolicy>,
    ) -> Result<EncodedInvocation> {
        self.submit_with_as(Source::for_callback(&self.entry), encoding_memory_bytes, encode, false, retry)
    }

    // A resident control runs until cancelled; it holds no native call slot.
    fn submit_with_as(
        &self, source: Source, encoding_memory_bytes: std::num::NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>,
        resident: bool, retry: Option<super::RetryPolicy>,
    ) -> Result<EncodedInvocation> {
        let _ = resident;
        let submitted = self.operations.clock().now();
        let origin = source.check()?;
        self.entry.available()?;
        self.entry.authorize(&origin)?;
        let timeout = self.entry.declared().policy.timeout;
        budget(self.operations.clock(), self.entry.id(), 1, timeout, submitted)?;
        let preparation = self.operations.claim_preparation(encoding_memory_bytes)?;

        let payload = catch_unwind(AssertUnwindSafe(|| encode(encoding_memory_bytes.get())))
            .map_err(|_| Error::OperationPanicked { operation: self.entry.id().to_owned() })??;
        if payload.capacity() > encoding_memory_bytes.get() {
            return Err(Error::ResourceExhausted { resource: "callback encoding claim" });
        }

        source.check()?;
        self.entry.available()?;
        budget(self.operations.clock(), self.entry.id(), 1, timeout, submitted)?;
        let dispatch = self.entry.wire.ok_or(Error::InvalidInput {
            field: "callback codec",
            reason: "callback target has no original codec binding",
        })?;
        dispatch(
            &self.operations,
            Submission {
                submitted,
                entry: Arc::clone(&self.entry),
                cause: None,
                preparation: None,
            },
            source,
            None,
            EncodedInput {
                retry,
                payload,
                preparation: Some(preparation),
            },
        )
    }
}

#[cfg(feature = "plugin-guest")]
impl super::Caller {
    pub(crate) fn submit_encoded_at(
        &self, operation: &str, identity: ([u8; 32], u64), payload: Vec<u8>, submitted: tokio::time::Instant,
        values: Vec<super::CallValue>, retry: Option<super::RetryPolicy>,
    ) -> Result<EncodedInvocation> {
        let context = self.operations.call_context().with_values(values)?;
        self.operations.with_context(context).submit_encoded_at_as(
            operation,
            payload,
            Source::Plugin(self.source.clone()),
            None,
            submitted,
            EncodedOptions { expected: Some(identity), retry },
        )
    }
}

impl super::Context<'_> {
    pub fn submit_encoded_with(&self, operation: &str, payload: Vec<u8>, retry: Option<super::RetryPolicy>) -> Result<EncodedInvocation> {
        self.operations.submit_encoded_at_as(
            operation,
            payload,
            self.source.clone(),
            Some(Parent {
                scope: &self.scope,
                ancestors: self.admission.held(),
            }),
            self.operations.clock().now(),
            EncodedOptions { expected: None, retry },
        )
    }

    pub fn submit_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<EncodedInvocation> {
        self.operations.submit_encoded_as(
            operation,
            payload,
            self.source.clone(),
            Some(Parent {
                scope: &self.scope,
                ancestors: self.admission.held(),
            }),
        )
    }

    #[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
    pub(crate) fn submit_encoded_at(
        &self, operation: &str, identity: ([u8; 32], u64), payload: Vec<u8>, submitted: tokio::time::Instant,
        values: Vec<super::CallValue>, retry: Option<super::RetryPolicy>,
    ) -> Result<EncodedInvocation> {
        let context = self.operations.call_context().with_values(values)?;
        self.operations.with_context(context).submit_encoded_at_as(
            operation,
            payload,
            self.source.clone(),
            Some(Parent {
                scope: &self.scope,
                ancestors: self.admission.held(),
            }),
            submitted,
            EncodedOptions { expected: Some(identity), retry },
        )
    }

    pub async fn call_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.submit_encoded(operation, payload)?.await
    }

    pub(super) fn submit_encoded_target(
        &self, entry: Arc<Entry>, payload: Vec<u8>, retry: Option<super::RetryPolicy>,
    ) -> Result<EncodedInvocation> {
        self.operations.dispatch_encoded(
            entry,
            payload,
            self.source.clone(),
            Some(Parent {
                scope: &self.scope,
                ancestors: self.admission.held(),
            }),
            self.operations.clock().now(),
            EncodedOptions { expected: None, retry },
        )
    }
}

/* === Public Functions === */

pub(super) fn dispatch_native(
    operations: &Manager, mut submission: Submission, source: Source, parent: Option<Parent<'_>>, input: EncodedInput,
) -> Result<EncodedInvocation> {
    let submitted = submission.submitted;
    let timeout = submission.entry.declared().policy.timeout;
    budget(operations.clock(), submission.entry.id(), 1, timeout, submitted)?;
    if let Some(binding) = submission.entry.binding.downcast_ref::<crate::plugin::grpc::invoke::Binding>() {
        let prepared = harmos_operations::execution::__private::PreparedNative {
            instance: binding.prepare(input.payload),
            grant: None,
            streams: None,
        };
        let mut request = super::binding::Request::from(Vec::new());
        request.retry = input.retry;
        return dispatch_prepared(operations, submission, source, parent, prepared, request);
    }
    let factory = submission.entry.binding.downcast_ref::<harmos_operations::execution::operations::OperationFactory>();
    let name = submission.entry.id().to_owned();
    submission.preparation = match input.preparation {
        Some(original) => Some(original),
        None => Some(operations.claim_preparation(std::num::NonZeroUsize::new(input.payload.capacity().max(1)).unwrap())?),
    };
    let decoding_bytes = catch_unwind(AssertUnwindSafe(|| {
        if let Some(factory) = factory {
            return factory.decoding_memory(&input.payload);
        }
        #[cfg(feature = "plugin-guest")]
        if let Some(binding) = submission.entry.binding.downcast_ref::<super::EncodedBinding>() {
            return binding.decoding_memory(&input.payload);
        }
        Err(crate::MessageError::borrowed("entry does not own a native codec"))
    }))
    .map_err(|_| Error::OperationPanicked { operation: name.clone() })?
    .map_err(|source| Error::OperationPayload { operation: name.clone(), source })?;
    if let Some(bytes) = std::num::NonZeroUsize::new(decoding_bytes) {
        let decoded = operations.claim_preparation(bytes)?;
        if let Some(original) = &mut submission.preparation {
            original.merge(decoded);
        } else {
            submission.preparation = Some(decoded);
        }
    }
    let resources = operations.resources_for(&submission.entry);
    let prepared = catch_unwind(AssertUnwindSafe(|| {
        if let Some(factory) = factory {
            return factory.prepare_encoded(resources, &input.payload);
        }
        #[cfg(feature = "plugin-guest")]
        if let Some(binding) = submission.entry.binding.downcast_ref::<super::EncodedBinding>() {
            return binding.prepare(resources, &input.payload);
        }
        Err(harmos_operations::execution::ServingError::Catalog)
    }))
    .map_err(|_| Error::OperationPanicked { operation: name.clone() })?
    .map_err(|source| Error::OperationFailed {
        operation: name,
        source: Arc::new(source),
    })?;
    drop(input.payload);
    let mut request = super::binding::Request::from(Vec::new());
    request.retry = input.retry;
    dispatch_prepared(operations, submission, source, parent, prepared, request)
}

pub(super) fn dispatch_prepared(
    operations: &Manager, submission: Submission, source: Source, parent: Option<Parent<'_>>,
    prepared: harmos_operations::execution::__private::PreparedNative, mut request: super::binding::Request<Vec<u8>>,
) -> Result<EncodedInvocation> {
    let submitted = submission.submitted;
    let timeout = request.policy.unwrap_or(submission.entry.declared().policy).timeout;
    let streaming = submission.entry.contract.as_ref().is_some_and(|contract| contract.input().is_some() || contract.output().is_some());
    let harmos_operations::execution::__private::PreparedNative { instance, grant, .. } = prepared;
    let instance = std::sync::Mutex::new(Some(instance));
    let binding = super::authored::factory_with::<Encoded, ()>(operations, move |_, ()| {
        instance.lock().expect("native preparation lock is healthy").take().ok_or(harmos_operations::execution::ServingError::Uninitialized)
    });
    request.binding = Some(Arc::new(binding));
    request.grant = grant;
    let operation = submission.entry.id().to_owned();
    let target = Arc::downgrade(&submission.entry);
    budget(operations.clock(), &operation, 1, timeout, submitted)?;
    let (scope, ancestors) = parent.map_or((None, &[][..]), |parent| (Some(parent.scope), parent.ancestors));
    let invocation = operations.admit_entry::<Encoded, ()>((request, ()), scope, ancestors, &source, submission)?;
    let id = invocation.id();
    let generation = invocation.generation();
    let control = Arc::clone(&invocation.control);
    let invocation = invocation.unguarded();
    let outcome_control = Arc::clone(&control);
    let clock = operations.clock().clone();
    let outcome = Box::pin(async move {
        let response = invocation.await?;
        available(&target, &source)?;
        if !streaming {
            budget(&clock, &operation, outcome_control.status().attempt() + 1, timeout, submitted)?;
        }
        Ok(response.try_unwrap().unwrap_or_else(|response| response.to_vec()))
    });
    Ok(EncodedInvocation { id, generation, outcome, control })
}

pub(super) fn available(target: &Weak<Entry>, source: &Source) -> Result<()> {
    let origin = source.check()?;
    target.upgrade().ok_or(Error::Closing { domain: "operation" })?.available_to(&origin)
}

pub(super) fn budget(
    clock: &crate::clock::Clock, operation: &str, attempt: usize, timeout: Duration, submitted: tokio::time::Instant,
) -> Result<()> {
    if clock.now().saturating_duration_since(submitted) >= timeout {
        return Err(Refusal::TimedOut {
            operation: operation.into(),
            attempt,
            timeout,
            configured_by: "operation policy.timeout",
        }
        .into());
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::RegistrationExt;

    #[crate::lanes]
    enum Lanes {
        #[default]
        Default,
    }

    struct Model;
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl crate::Application for Model {
        type Lanes = Lanes;
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
        type Streams = ();
    }

    #[crate::operation(harmos_path = crate, resources = [], id = "fenced.probe", response = u64,
        policy = crate::OperationPolicy {
            timeout: Duration::from_secs(1),
            attempt_timeout: Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: crate::RetryPolicy::Never,
        },
    )]
    struct Probe {
        value: u64,
    }

    impl crate::Operation for Probe {
        type Error = std::io::Error;

        async fn run(self, _: crate::Context<'_, Self>) -> std::io::Result<crate::Response<u64>> {
            Ok(self.value.into())
        }
    }

    #[cfg(feature = "grpc")]
    #[tokio::test]
    async fn private_external_calls_are_refused() {
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.settings().operation_settings(crate::operations::Settings {
                preparation_bytes: std::num::NonZeroUsize::new(4096),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        let batch = harmos_operations::execution::__private::RegistrationBatch::new().operation::<Probe>();
        runtime.register_batch(batch.lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();

        let source = Source::Host.external().unwrap();
        assert!(matches!(
            runtime.operations.admit::<Probe, ()>((Probe { value: 7 }.into(), ()), None, &[], &source, None),
            Err(Error::OperationUnauthorized { .. })
        ));
        assert!(matches!(
            runtime.operations.submit_encoded_as("host.fenced.probe", Vec::new(), source, None),
            Err(Error::OperationUnauthorized { .. })
        ));
        assert_eq!(*runtime.operation(Probe { value: 7 }).await.unwrap(), 7);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn assigned_identity_fences_before_decoding() {
        let runtime = crate::Runtime::<Model>::launch((), ()).await.unwrap();
        let batch = harmos_operations::execution::__private::RegistrationBatch::new().operation::<Probe>();
        runtime.register_batch(batch.clone().lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
        let entry = runtime.operations.entry(Probe::ID).unwrap();
        let fingerprint = entry.declared().fingerprint.0;
        let generation = entry.generation.value();
        let mut wrong = fingerprint;
        wrong[0] ^= 1;

        let submit = |fingerprint, generation| {
            runtime.operations.submit_encoded_at_as(
                "host.fenced.probe",
                Vec::new(),
                Source::Host,
                None,
                executor::now(),
                EncodedOptions {
                    expected: Some((fingerprint, generation)),
                    retry: None,
                },
            )
        };
        assert!(matches!(submit(wrong, generation), Err(Error::InvalidInput { field: "encoded operation identity", .. })));
        assert!(matches!(submit(fingerprint, generation), Err(Error::InvalidInput { field: "operation preparation bytes", .. })));

        let id = crate::QualifiedId::parse("host.fenced.probe").unwrap();
        runtime.unregister(&id).await.unwrap();
        runtime.register_batch(batch.lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
        assert!(matches!(submit(fingerprint, generation), Err(Error::InvalidInput { field: "encoded operation identity", .. })));
        runtime.shutdown().await.unwrap();
    }
}
