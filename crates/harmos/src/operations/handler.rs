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
pub struct OperationInstanceId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationGeneration(u64);

pub struct OperationInstance<O: OperationDeclaration> {
    pub(crate) id: OperationInstanceId,
    pub(super) generation: OperationGeneration,
    pub(crate) outcome: oneshot::Receiver<Result<Arc<O::Response>>>,
    pub(super) control: Arc<crate::supervision::instance::Instance>,
}

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
impl OperationInstanceId {
    pub(crate) fn fresh() -> Result<Self> {
        NEXT_INSTANCE
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map(Self)
            .map_err(|_| Error::ResourceExhausted {
                resource: "operation instance identities",
            })
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

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

    pub async fn join(self) -> Result<Arc<O::Response>> {
        self.await
    }

    pub async fn settle(&self) -> Result<()> {
        self.control
            .settle()
            .await
            .map_err(|source| Error::OperationSettlement { source })
    }
}

impl<O: OperationDeclaration> Future for OperationInstance<O> {
    type Output = Result<Arc<O::Response>>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.get_mut().outcome).poll(cx) {
            Poll::Ready(Ok(answer)) => Poll::Ready(answer),
            Poll::Ready(Err(_)) => Poll::Ready(Err(crate::Refusal::Unavailable.into())),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Context<'_> {
    pub fn context(&self) -> &super::CallContext {
        self.operations.call_context()
    }

    // Derive a child caller while retaining this attempt's identity, admission,
    // cancellation and deadline. Only the selected portable value changes.
    pub fn with_value<T: super::ContextValue>(mut self, value: T) -> Result<Self> {
        let context = self.context().with_value(value)?;
        self.operations = self.operations.with_context(context);
        Ok(self)
    }

    pub fn id(&self) -> OperationInstanceId {
        self.id
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

    pub fn submit<O: OperationDeclaration>(
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
            (super::binding::Request::charged(request), endpoints),
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
    ) -> Result<Arc<O::Response>> {
        self.submit_caused::<O>(request, cause)?.await
    }

    pub async fn call<O: OperationDeclaration>(
        &self,
        request: O::Request,
    ) -> Result<Arc<O::Response>> {
        self.submit::<O>(request)?.await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
