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
pub use harmos_contract::operations::instance::Id as OperationInstanceId;

/* === Imports === */
// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::task::{Context as TaskContext, Poll};
// Third Party Imports
use tokio::sync::oneshot;

// Local Imports
use super::OperationManager;
use super::admission::Admission;
use super::caller::{Origin, Source};
use crate::operations::OperationDeclaration;
use crate::{Error, Result, operations::Scope, state::Position};

/* === Constants === */
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationGeneration(u64);

// Dropping an instance before its outcome is delivered requests cancellation;
// `detach` keeps the operation running without a handle.
#[must_use = "dropping an unsettled operation instance requests its cancellation"]
pub struct OperationInstance<O: OperationDeclaration> {
    pub(crate) id: OperationInstanceId,
    pub(super) generation: OperationGeneration,
    pub(crate) outcome: oneshot::Receiver<Result<crate::operations::Response<O::Response>>>,
    pub(super) control: Arc<crate::supervision::instance::Instance>,
    pub(super) released: bool,
}

#[derive(Clone)]
pub struct Context<'a> {
    pub(crate) id: OperationInstanceId,
    pub(crate) attempt: usize,
    pub(crate) scope: Scope<'a>,
    pub(crate) operations: OperationManager,
    pub(super) admission: Admission,
    pub(super) source: Source,
    pub(super) origin: Origin,
    #[cfg(feature = "plugin-sidecar")]
    pub(super) retirement: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationGeneration {
    pub(super) fn fresh() -> Result<Self> {
        NEXT_GENERATION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map(Self)
            .map_err(|_| Error::ResourceExhausted {
                resource: "operation provider generations",
            })
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

impl<O: OperationDeclaration> OperationInstance<O> {
    pub fn id(&self) -> OperationInstanceId {
        self.id
    }

    pub fn generation(&self) -> OperationGeneration {
        self.generation
    }

    pub fn cancel(&self) {
        self.control.cancel();
    }

    pub fn status(&self) -> super::OperationStatus {
        self.control.status()
    }

    pub async fn join(self) -> Result<crate::operations::Response<O::Response>> {
        self.await
    }

    pub async fn cancel_and_join(self) -> Result<crate::operations::Response<O::Response>> {
        self.cancel();
        self.await
    }

    // Keep the operation running without a handle until its submitting
    // attempt ends, which cancels and settles it like any other child.
    pub fn detach(mut self) -> OperationInstanceId {
        self.released = true;
        self.id
    }

    pub async fn settle(&self) -> Result<()> {
        self.control
            .settle()
            .await
            .map_err(|source| Error::OperationSettlement { source })
    }
}

impl<O: OperationDeclaration> OperationInstance<O> {
    // Encoded invocations keep their own contract: dropping one never
    // cancels, so the typed instance they await must not cancel either.
    pub(super) fn unguarded(mut self) -> Self {
        self.released = true;
        self
    }
}

impl<O: OperationDeclaration> Future for OperationInstance<O> {
    type Output = Result<crate::operations::Response<O::Response>>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let instance = self.get_mut();
        let answer = match Pin::new(&mut instance.outcome).poll(cx) {
            Poll::Ready(Ok(answer)) => answer,
            Poll::Ready(Err(_)) => Err(crate::Refusal::Unavailable.into()),
            Poll::Pending => return Poll::Pending,
        };
        instance.released = true;
        Poll::Ready(answer)
    }
}

impl<O: OperationDeclaration> Drop for OperationInstance<O> {
    fn drop(&mut self) {
        if !self.released && !self.control.is_settled() {
            self.control.cancel();
        }
    }
}

impl<O> harmos_contract::operations::Instance<O> for OperationInstance<O>
where
    O: OperationDeclaration,
{
    type Error = Error;

    fn id(&self) -> OperationInstanceId {
        self.id
    }

    fn status(
        &self,
    ) -> impl Future<Output = Result<harmos_contract::operations::instance::Status>> + Send {
        std::future::ready(Ok(self.control.status().portable()))
    }

    fn cancel(&self) -> impl Future<Output = Result<()>> + Send {
        self.control.cancel();
        std::future::ready(Ok(()))
    }

    fn join(self) -> impl Future<Output = Result<crate::operations::Response<O::Response>>> + Send {
        self
    }

    fn detach(self) -> OperationInstanceId {
        OperationInstance::detach(self)
    }
}

impl Context<'_> {
    pub fn context(&self) -> &super::CallContext {
        self.operations.call_context()
    }

    // Derive a child caller while retaining this attempt's identity, admission,
    // cancellation and deadline. Only the selected portable value changes.
    pub fn with_value<T: super::ContextValue>(&self, value: T) -> Result<Self> {
        let context = self.context().with_value(value)?;
        let mut child = self.clone();
        child.operations = child.operations.with_context(context);
        Ok(child)
    }

    pub fn id(&self) -> OperationInstanceId {
        self.id
    }

    // An active instance this attempt's caller submitted; others stay hidden.
    pub(crate) fn owned_instance(
        &self,
        id: OperationInstanceId,
    ) -> Option<Arc<crate::supervision::instance::Instance>> {
        self.operations.instances.owned(id, &self.origin.identity)
    }

    pub fn attempt(&self) -> usize {
        self.attempt
    }

    pub fn scope(&self) -> &Scope<'_> {
        &self.scope
    }

    pub fn is_host_call(&self) -> bool {
        self.origin.identity == super::caller::Identity::Host
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn caller_scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.origin.scope.as_ref()
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(crate) fn retains_publication(&self, scope: &crate::plugin::PluginScope) -> bool {
        self.source.check().is_ok_and(|origin| {
            (origin.has_lifecycle_loan() || origin.observer_publication)
                && origin
                    .scope
                    .as_ref()
                    .is_some_and(|caller| caller.same_activation(scope))
        })
    }

    pub fn submit_at<O: OperationDeclaration>(
        &self,
        id: &str,
        request: O::Request,
    ) -> Result<OperationInstance<O>> {
        self.operations.admit_at::<O, ()>(
            id,
            (request.into(), ()),
            Some(&self.scope),
            self.admission.held(),
            &self.source,
            None,
        )
    }

    // The declaration is its own request; `submit_as::<O>` names a separate one.
    pub fn submit<O: OperationDeclaration<Request = O>>(
        &self,
        request: O,
    ) -> Result<OperationInstance<O>> {
        self.submit_as::<O>(request)
    }

    pub async fn call<O: OperationDeclaration<Request = O>>(
        &self,
        request: O,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.call_as::<O>(request).await
    }
    pub fn submit_as<O: OperationDeclaration>(
        &self,
        request: O::Request,
    ) -> Result<OperationInstance<O>> {
        self.operations.submit_scoped::<O, ()>(
            (request.into(), ()),
            &self.scope,
            &self.admission,
            &self.source,
            None,
        )
    }

    pub fn submit_endpoints<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: O::Request,
        endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.operations.submit_scoped::<O, P>(
            (request.into(), endpoints),
            &self.scope,
            &self.admission,
            &self.source,
            None,
        )
    }

    pub fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: super::StreamItem<O::Request>,
        endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.operations.submit_scoped::<O, P>(
            (super::binding::Request::charged(request)?, endpoints),
            &self.scope,
            &self.admission,
            &self.source,
            None,
        )
    }

    pub fn submit_caused<O: OperationDeclaration>(
        &self,
        request: O::Request,
        cause: Position,
    ) -> Result<OperationInstance<O>> {
        self.operations.submit_scoped::<O, ()>(
            (request.into(), ()),
            &self.scope,
            &self.admission,
            &self.source,
            Some(cause),
        )
    }

    pub async fn call_caused<O: OperationDeclaration>(
        &self,
        request: O::Request,
        cause: Position,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.submit_caused::<O>(request, cause)?.await
    }

    pub async fn call_as<O: OperationDeclaration>(
        &self,
        request: O::Request,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.submit_as::<O>(request)?.await
    }
}

/* === Public Functions === */
pub(crate) fn fresh_instance() -> Result<OperationInstanceId> {
    NEXT_INSTANCE
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map(OperationInstanceId::new)
        .map_err(|_| Error::ResourceExhausted {
            resource: "operation instance identities",
        })
}

/* === Private Functions === */

/* === Tests === */
