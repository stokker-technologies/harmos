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
use super::{OperationInstanceId, OperationManager};
use crate::operations::OperationDeclaration;
use crate::supervision::executor;
use crate::{Error, Message, Refusal, Result, operations::Scope};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Outcome = Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send>>;
pub(super) type Dispatch = fn(&OperationManager, Submission, Source, Option<Parent<'_>>, EncodedInput) -> Result<EncodedInvocation>;

/* === Structs === */

pub(super) struct EncodedInput {
    pub(super) payload: Vec<u8>,
    pub(super) preparation: Option<OwnedSemaphorePermit>,

    #[cfg(feature = "plugin-sidecar")]
    pub(super) native_capacity: Option<OwnedSemaphorePermit>,
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
#[derive(Clone)]
pub struct PluginCallback {
    operations: OperationManager,
    entry: Arc<Entry>,
    contract: crate::operations::Contract,
}

#[cfg(feature = "plugin-sidecar")]
#[derive(Clone)]
pub(crate) struct PluginControl(PluginCallback);

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

impl EncodedInvocation {
    pub fn id(&self) -> OperationInstanceId {
        self.id
    }

    pub fn generation(&self) -> super::OperationGeneration {
        self.generation
    }

    pub fn cancel(&self) {
        self.control.cancel();
    }

    pub fn status(&self) -> super::OperationStatus {
        self.control.status()
    }

    pub async fn join(self) -> Result<Vec<u8>> {
        self.await
    }

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

impl OperationManager {
    pub fn submit_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<EncodedInvocation> {
        self.submit_encoded_as(operation, payload, Source::Host, None)
    }

    pub async fn call_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.submit_encoded(operation, payload)?.await
    }

    pub(super) fn submit_encoded_as(
        &self, operation: &str, payload: Vec<u8>, source: Source, parent: Option<Parent<'_>>,
    ) -> Result<EncodedInvocation> {
        self.submit_encoded_at_as(operation, payload, source, parent, executor::now())
    }

    fn submit_encoded_at_as(
        &self, operation: &str, payload: Vec<u8>, source: Source, parent: Option<Parent<'_>>, submitted: tokio::time::Instant,
    ) -> Result<EncodedInvocation> {
        let origin = source.check()?;
        let entry = self.entry_as(operation, &origin)?;
        entry.authorize(&origin)?;
        let dispatch = entry
            .wire
            .ok_or(Error::InvalidInput { field: "encoded operation", reason: "operation has no cross-boundary codec binding" })?;

        dispatch(
            self,
            Submission { submitted, entry, cause: None, preparation: None },
            source,
            parent,
            EncodedInput {
                payload,
                preparation: None,
                #[cfg(feature = "plugin-sidecar")]
                native_capacity: None,
            },
        )
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl OperationManager {
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn capture_host_control(
        &self, scope: &crate::plugin::PluginScope, contract: &crate::operations::Contract,
    ) -> Result<PluginControl> {
        if contract.operation().visibility != crate::operations::Visibility::Host || scope.observe_contract() != Some(contract) {
            return Err(Error::InvalidInput { field: "observer control", reason: "control differs from the frozen Host contract" });
        }
        self.capture_callback(scope, contract).map(PluginControl)
    }

    pub fn capture_callback(&self, scope: &crate::plugin::PluginScope, contract: &crate::operations::Contract) -> Result<PluginCallback> {
        if scope.is_closed() {
            return Err(Error::Closing { domain: "callback scope" });
        }
        let entry = self.capture_provider(contract.operation().id.as_str(), Some(scope.owner()))?;
        let selected = entry
            .plugin()
            .ok_or(Error::InvalidInput { field: "callback scope", reason: "callback target has no original plugin generation" })?;
        if !selected.same_activation(scope) {
            return Err(Error::InvalidInput {
                field: "callback scope",
                reason: "callback target belongs to a different plugin generation",
            });
        }
        PluginCallback::from_entry(self.clone(), entry, contract)
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl PluginCallback {
    pub(super) fn from_entry(operations: OperationManager, entry: Arc<Entry>, contract: &crate::operations::Contract) -> Result<Self> {
        if entry.declared() != contract.operation() || entry.contract.as_ref() != Some(contract) {
            return Err(Error::InvalidInput {
                field: "callback contract",
                reason: "callback requires the exact retained canonical contract",
            });
        }
        if entry.wire.is_none() {
            return Err(Error::InvalidInput { field: "callback codec", reason: "callback target has no original codec binding" });
        }
        Ok(Self { operations, entry, contract: contract.clone() })
    }

    pub fn contract(&self) -> &crate::operations::Contract {
        &self.contract
    }

    pub fn submit_with(
        &self, encoding_memory_bytes: std::num::NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>,
    ) -> Result<EncodedInvocation> {
        self.submit_with_as(Source::for_callback(&self.entry), encoding_memory_bytes, encode, false)
    }

    // A resident control runs until cancelled; it holds no native call slot.
    fn submit_with_as(
        &self, source: Source, encoding_memory_bytes: std::num::NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>, resident: bool,
    ) -> Result<EncodedInvocation> {
        #[cfg(not(feature = "plugin-sidecar"))]
        let _ = resident;
        let submitted = executor::now();
        let origin = source.check()?;
        self.entry.available()?;
        self.entry.authorize(&origin)?;
        let timeout = self.entry.declared().policy.timeout;
        budget(timeout, submitted)?;
        let preparation = self.operations.claim_preparation(encoding_memory_bytes)?;
        #[cfg(feature = "plugin-sidecar")]
        let native_capacity = self
            .entry
            .native
            .as_ref()
            .map(|binding| if resident { Ok(super::native::Binding::resident_slot()) } else { binding.reserve() })
            .transpose()
            .map_err(|source| Error::OperationFailed { operation: self.entry.id().to_owned(), source: Arc::new(source) })?;
        let payload = catch_unwind(AssertUnwindSafe(|| encode(encoding_memory_bytes.get())))
            .map_err(|_| Error::OperationPanicked { operation: self.entry.id().to_owned() })??;
        if payload.capacity() > encoding_memory_bytes.get() {
            return Err(Error::ResourceExhausted { resource: "callback encoding claim" });
        }
        #[cfg(feature = "plugin-sidecar")]
        if native_capacity.is_some() && payload.capacity() > harmos_sidecar::Frame::LIMIT {
            return Err(Error::ResourceExhausted { resource: "native callback frame" });
        }
        source.check()?;
        self.entry.available()?;
        budget(timeout, submitted)?;
        let dispatch = self
            .entry
            .wire
            .ok_or(Error::InvalidInput { field: "callback codec", reason: "callback target has no original codec binding" })?;
        dispatch(
            &self.operations,
            Submission { submitted, entry: Arc::clone(&self.entry), cause: None, preparation: None },
            source,
            None,
            EncodedInput {
                payload,
                preparation: Some(preparation),
                #[cfg(feature = "plugin-sidecar")]
                native_capacity,
            },
        )
    }
}

#[cfg(feature = "plugin-sidecar")]
impl PluginControl {
    pub(crate) fn submit_with(
        &self, bytes: std::num::NonZeroUsize, encode: impl FnOnce(usize) -> Result<Vec<u8>>,
    ) -> Result<EncodedInvocation> {
        self.0.submit_with_as(Source::Host, bytes, encode, true)
    }
}

#[cfg(any(feature = "plugin-sidecar", feature = "plugin-guest"))]
impl super::Caller {
    pub(crate) fn submit_encoded_at(
        &self, operation: &str, payload: Vec<u8>, submitted: tokio::time::Instant, values: Vec<super::CallValue>,
    ) -> Result<EncodedInvocation> {
        let context = self.operations.call_context().with_values(values)?;
        self.operations.with_context(context).submit_encoded_at_as(operation, payload, Source::Plugin(self.source.clone()), None, submitted)
    }
}

impl super::Context<'_> {
    pub fn submit_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<EncodedInvocation> {
        self.operations.submit_encoded_as(
            operation,
            payload,
            self.source.clone(),
            Some(Parent { scope: &self.scope, ancestors: self.admission.held() }),
        )
    }

    #[cfg(any(feature = "plugin-sidecar", all(feature = "plugin-guest", not(target_arch = "wasm32"))))]
    pub(crate) fn submit_encoded_at(
        &self, operation: &str, payload: Vec<u8>, submitted: tokio::time::Instant, values: Vec<super::CallValue>,
    ) -> Result<EncodedInvocation> {
        let context = self.operations.call_context().with_values(values)?;
        self.operations.with_context(context).submit_encoded_at_as(
            operation,
            payload,
            self.source.clone(),
            Some(Parent { scope: &self.scope, ancestors: self.admission.held() }),
            submitted,
        )
    }

    pub async fn call_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.submit_encoded(operation, payload)?.await
    }
}

/* === Public Functions === */

pub(super) fn dispatch<O>(
    operations: &OperationManager, mut submission: Submission, source: Source, parent: Option<Parent<'_>>, input: EncodedInput,
) -> Result<EncodedInvocation>
where
    O: OperationDeclaration,
{
    let submitted = submission.submitted;
    let timeout = submission.entry.declared().policy.timeout;
    budget(timeout, submitted)?;
    if operations.preparation.is_none() {
        return Err(Error::InvalidInput {
            field: "operation preparation bytes",
            reason: "encoded preparation requires an explicit preparation budget",
        });
    }
    submission.preparation = match input.preparation {
        Some(original) => Some(original),
        None => std::num::NonZeroUsize::new(input.payload.capacity()).map(|bytes| operations.claim_preparation(bytes)).transpose()?,
    };
    let decoding_bytes = codec::<O, _>(|| <O as crate::Message>::decoding_memory(&input.payload))?
        .map_err(|source| Error::OperationPayload { operation: O::ID.to_owned(), source })?;
    if let Some(bytes) = std::num::NonZeroUsize::new(decoding_bytes) {
        let decoded = operations.claim_preparation(bytes)?;
        if let Some(original) = &mut submission.preparation {
            original.merge(decoded);
        } else {
            submission.preparation = Some(decoded);
        }
    }
    budget(timeout, submitted)?;
    let request = codec::<O, _>(|| <O as crate::Message>::decode(&input.payload))?
        .map_err(|source| Error::OperationPayload { operation: O::ID.to_owned(), source })?;
    let target = Arc::downgrade(&submission.entry);
    budget(timeout, submitted)?;
    let (scope, ancestors) = parent.map_or((None, &[][..]), |parent| (Some(parent.scope), parent.ancestors));
    let invocation = operations.admit_entry::<O, ()>((request.into(), ()), scope, ancestors, &source, submission)?;
    let id = invocation.id();
    let generation = invocation.generation();
    let control = Arc::clone(&invocation.control);
    let invocation = invocation.unguarded();
    let outcome = Box::pin(async move {
        let response = invocation.await?;
        available(&target, &source)?;
        budget(timeout, submitted)?;
        let payload = codec::<O, _>(|| response.encode_to_vec())?;
        available(&target, &source)?;
        budget(timeout, submitted)?;

        Ok(payload)
    });

    Ok(EncodedInvocation { id, generation, outcome, control })
}

pub(super) fn available(target: &Weak<Entry>, source: &Source) -> Result<()> {
    let origin = source.check()?;
    target.upgrade().ok_or(Error::Closing { domain: "operation" })?.available_to(&origin)
}

pub(super) fn budget(timeout: Duration, submitted: tokio::time::Instant) -> Result<()> {
    if executor::now().saturating_duration_since(submitted) >= timeout {
        return Err(Refusal::TimedOut.into());
    }
    Ok(())
}

/* === Private Functions === */

fn codec<O: OperationDeclaration, T>(run: impl FnOnce() -> T) -> Result<T> {
    catch_unwind(AssertUnwindSafe(run)).map_err(|_| Error::OperationPanicked { operation: O::ID.to_owned() })
}

/* === Tests === */
