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

pub use harmos_domain::operations::instance::Id as OperationInstanceId;

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
use super::Manager;
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
// `detach` keeps the operation running without a handle. A resident handle
// (an observer or service activation) holds one lease on its shared instance:
// dropping it releases that lease and the last release stops the instance.
#[must_use = "dropping an unsettled operation instance requests its cancellation"]
pub struct Running<O: OperationDeclaration> {
    pub(crate) id: OperationInstanceId,
    pub(super) generation: OperationGeneration,
    pub(crate) outcome: Outcome<O::Response>,
    pub(super) control: Arc<crate::supervision::instance::Instance>,
    pub(super) released: bool,
}

pub(crate) enum Outcome<T: Send + Sync + 'static> {
    Request(oneshot::Receiver<Result<crate::operations::Response<T>>>),
    Resident {
        lease: Option<super::resident::Lease>,
        // Polled only through `&mut`; the lock makes a shared handle `Sync`.
        settled: std::sync::Mutex<Settling<T>>,
    },
}

pub(crate) type Settling<T> = Pin<Box<dyn Future<Output = Result<crate::operations::Response<T>>> + Send>>;

#[derive(Clone)]
pub struct Context<'a> {
    pub(crate) id: OperationInstanceId,
    pub(crate) attempt: usize,
    pub(crate) generation: OperationGeneration,
    pub(crate) scope: Scope<'a>,
    pub(crate) operations: Manager,
    pub(super) admission: Admission,
    pub(super) source: Source,
    pub(super) origin: Origin,

    #[cfg(feature = "plugin-sidecar")]
    pub(super) retirement: bool,
}

#[derive(Clone)]
pub struct HostContext {
    scope: super::scope::ScopeCore,
    id: OperationInstanceId,
    attempt: usize,
    generation: OperationGeneration,
    operations: Manager,
    admission: Admission,
    source: Source,
    origin: Origin,
    #[cfg(feature = "plugin-sidecar")]
    retirement: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl OperationGeneration {
    pub(super) fn fresh() -> Result<Self> {
        NEXT_GENERATION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map(Self)
            .map_err(|_| Error::ResourceExhausted { resource: "operation provider generations" })
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

impl<O: OperationDeclaration> Running<O> {
    pub fn id(&self) -> OperationInstanceId {
        self.id
    }

    pub fn generation(&self) -> OperationGeneration {
        self.generation
    }

    // A resident instance stops for every lease holder.
    pub fn cancel(&self) {
        match &self.outcome {
            Outcome::Resident { lease: Some(lease), .. } => lease.stop(),
            _ => self.control.cancel(),
        }
    }

    pub fn status(&self) -> super::OperationStatus {
        self.control.status()
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<super::OperationStatus> {
        self.control.settlement()
    }

    // Wait until the instance runs; a resident that stops or fails first
    // reports why once its cleanup settles.
    pub async fn ready(&self) -> Result<()> {
        let mut status = self.control.settlement();
        loop {
            let current = status.borrow_and_update().clone();
            match current.phase() {
                super::OperationPhase::Running => return Ok(()),
                phase if phase.is_terminal() || current.cancellation_requested() || current.outcome().is_some() => {
                    self.settle().await?;
                    return Err(Error::NotReady { domain: "operation activation" });
                }
                _ => {}
            }
            if status.changed().await.is_err() {
                return Err(Error::NotReady { domain: "operation activation" });
            }
        }
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.join", level = "debug", skip_all)]
    pub async fn join(self) -> Result<crate::operations::Response<O::Response>> {
        self.await
    }

    pub async fn cancel_and_join(self) -> Result<crate::operations::Response<O::Response>> {
        self.cancel();
        self.await
    }

    // Keep the operation running without a handle until its submitting
    // attempt ends, which cancels and settles it like any other child. A
    // detached resident lease is held until its resources retire.
    pub fn detach(mut self) -> OperationInstanceId {
        self.released = true;
        if let Outcome::Resident { lease, .. } = &mut self.outcome
            && let Some(lease) = lease.take()
        {
            lease.detach();
        }
        self.id
    }

    // Release this handle. A resident lease that was the last one stops its
    // instance and waits for it; a request-scoped instance is cancelled and
    // settled.
    pub async fn release(mut self) -> Result<()> {
        self.released = true;
        let lease = match &mut self.outcome {
            Outcome::Resident { lease, .. } => lease.take(),
            Outcome::Request(_) => None,
        };
        match lease {
            Some(lease) => lease.release().await,
            None => {
                self.control.cancel();
                self.settle().await
            }
        }
    }

    // Wait for the instance to settle without consuming the handle. A
    // resident reports its outcome and retries a retained cleanup.
    #[tracing::instrument(target = "harmos::operations", name = "operations.settle", level = "debug", skip_all)]
    pub async fn settle(&self) -> Result<()> {
        if let Outcome::Resident { lease: Some(lease), .. } = &self.outcome {
            return lease.settlement().await;
        }
        self.control.settle().await.map_err(|source| Error::OperationSettlement { source })
    }
}

impl<O: OperationDeclaration> Running<O> {
    // Encoded invocations keep their own contract: dropping one never
    // cancels, so the typed instance they await must not cancel either.
    pub(super) fn unguarded(mut self) -> Self {
        self.released = true;
        self
    }
}

impl<O: OperationDeclaration> Future for Running<O> {
    type Output = Result<crate::operations::Response<O::Response>>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let instance = self.get_mut();
        let answer = match &mut instance.outcome {
            Outcome::Request(outcome) => match Pin::new(outcome).poll(cx) {
                Poll::Ready(Ok(answer)) => answer,
                Poll::Ready(Err(_)) => Err(crate::Refusal::Unavailable.into()),
                Poll::Pending => return Poll::Pending,
            },
            Outcome::Resident { settled, .. } => match settled.get_mut().expect("resident settlement is healthy").as_mut().poll(cx) {
                Poll::Ready(answer) => answer,
                Poll::Pending => return Poll::Pending,
            },
        };
        instance.released = true;
        Poll::Ready(answer)
    }
}

impl<O: OperationDeclaration> Drop for Running<O> {
    fn drop(&mut self) {
        // A resident lease releases itself when dropped.
        if matches!(self.outcome, Outcome::Request(_)) && !self.released && !self.control.is_settled() {
            self.control.cancel();
        }
    }
}

impl<O> harmos_sdk::operations::Instance<O> for Running<O>
where
    O: OperationDeclaration,
{
    type Error = Error;

    fn id(&self) -> OperationInstanceId {
        self.id
    }

    fn status(&self) -> impl Future<Output = Result<harmos_domain::operations::instance::Status>> + Send {
        std::future::ready(Ok(self.control.status().portable()))
    }

    fn cancel(&self) -> impl Future<Output = Result<()>> + Send {
        Running::cancel(self);
        std::future::ready(Ok(()))
    }

    fn join(self) -> impl Future<Output = Result<crate::operations::Response<O::Response>>> + Send {
        self
    }

    fn detach(self) -> OperationInstanceId {
        Running::detach(self)
    }
}

impl Context<'_> {
    pub(crate) fn owned(&self) -> HostContext {
        HostContext {
            scope: self.scope.owned(),
            id: self.id,
            attempt: self.attempt,
            generation: self.generation,
            operations: self.operations.clone(),
            admission: self.admission.clone(),
            source: self.source.clone(),
            origin: self.origin.clone(),
            #[cfg(feature = "plugin-sidecar")]
            retirement: self.retirement,
        }
    }

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
    pub(crate) fn owned_instance(&self, id: OperationInstanceId) -> Option<Arc<crate::supervision::instance::Instance>> {
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

    // Revalidate The Caller Before Every Streamed Destination
    pub(crate) fn admit_stream_grant(&self, grant: &harmos_domain::plugin::Grant) -> Result<()> {
        self.source.check()?;
        match &self.origin.grants {
            Some(authority) => authority.admit(Some(grant)),
            None if self.is_host_call() => Ok(()),
            None => Err(Error::PermissionDenied),
        }
    }

    // Refuse a provider-prepared grant the admitted caller does not hold.
    #[cfg(any(all(feature = "plugin-guest", not(target_arch = "wasm32")), feature = "plugin-sidecar"))]
    pub(super) fn admit_grant(&self, grant: Option<&harmos_domain::plugin::Grant>) -> Result<()> {
        match (&self.origin.grants, grant) {
            (Some(authority), grant) => authority.admit(grant),
            (None, None) => Ok(()),
            (None, Some(_)) => Err(Error::PermissionDenied),
        }
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub fn caller_scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.origin.scope.as_ref()
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) fn retains_publication(&self, scope: &crate::plugin::PluginScope) -> bool {
        self.source.check().is_ok_and(|origin| {
            (origin.has_lifecycle_loan() || origin.resident) && origin.scope.as_ref().is_some_and(|caller| caller.same_activation(scope))
        })
    }

    pub fn submit_at<O: OperationDeclaration>(&self, id: &str, request: O::Request) -> Result<Running<O>> {
        self.operations.admit_at::<O, ()>(id, (request.into(), ()), Some(&self.scope), self.admission.held(), &self.source, None)
    }

    pub fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<Running<O>> {
        self.operations.submit_scoped::<O, ()>((request.into(), ()), &self.scope, &self.admission, &self.source, None)
    }

    pub fn submit_endpoints<O: OperationDeclaration, P: Send + 'static>(&self, request: O::Request, endpoints: P) -> Result<Running<O>> {
        self.operations.submit_scoped::<O, P>((request.into(), endpoints), &self.scope, &self.admission, &self.source, None)
    }

    pub fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self, request: super::StreamItem<O::Request>, endpoints: P,
    ) -> Result<Running<O>> {
        self.operations.submit_scoped::<O, P>(
            (super::binding::Request::charged(request)?, endpoints),
            &self.scope,
            &self.admission,
            &self.source,
            None,
        )
    }

    pub fn submit_caused<O: OperationDeclaration>(&self, request: O::Request, cause: Position) -> Result<Running<O>> {
        self.operations.submit_scoped::<O, ()>((request.into(), ()), &self.scope, &self.admission, &self.source, Some(cause))
    }

    pub async fn call_caused<O: OperationDeclaration>(
        &self, request: O::Request, cause: Position,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.submit_caused::<O>(request, cause)?.await
    }

    pub async fn call<O: OperationDeclaration>(&self, request: O::Request) -> Result<crate::operations::Response<O::Response>> {
        self.submit::<O>(request)?.await
    }
}

/* === Public Functions === */

impl HostContext {
    pub fn borrow(&self) -> Context<'_> {
        Context {
            scope: Scope::over(&self.scope),
            id: self.id,
            attempt: self.attempt,
            generation: self.generation,
            operations: self.operations.clone(),
            admission: self.admission.clone(),
            source: self.source.clone(),
            origin: self.origin.clone(),
            #[cfg(feature = "plugin-sidecar")]
            retirement: self.retirement,
        }
    }
}

impl harmos_sdk::operations::Host for HostContext {
    fn publish_on<S: harmos_domain::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        use harmos_domain::Message;
        let published = self.call::<harmos_operations::streams::Publish>(harmos_operations::streams::PublishRequest {
            stream: S::ID.to_owned(),
            schema: S::VERSION,
            sequence: None,
            published_unix_micros: None,
            context: None,
            payload: row.encode(),
            following: Vec::new(),
        });
        async move {
            published.await?;
            Ok(())
        }
    }

    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;

    fn context(&self) -> &super::CallContext {
        self.operations.call_context()
    }

    fn with_value<T: super::ContextValue>(&self, value: T) -> std::result::Result<Self, super::ContextError> {
        let context = self.operations.call_context().with_value(value)?;
        let mut child = self.clone();
        child.operations = child.operations.with_context(context);
        Ok(child)
    }

    async fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<Running<O>> {
        self.borrow().submit::<O>(request)
    }

    async fn call<O: OperationDeclaration>(&self, request: O::Request) -> Result<super::Response<O::Response>> {
        self.borrow().submit::<O>(request)?.await
    }
}

pub(crate) fn fresh_instance() -> Result<OperationInstanceId> {
    NEXT_INSTANCE
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map(OperationInstanceId::new)
        .map_err(|_| Error::ResourceExhausted { resource: "operation instance identities" })
}

/* === Private Functions === */

/* === Tests === */

impl harmos_sdk::operations::instances::Instances for HostContext {
    fn owned(&self, id: OperationInstanceId) -> Option<Arc<dyn harmos_sdk::operations::instances::InstanceControl>> {
        self.borrow().owned_instance(id).map(|instance| instance as Arc<dyn harmos_sdk::operations::instances::InstanceControl>)
    }
}
impl harmos_sdk::operations::instances::InstanceControl for crate::supervision::instance::Instance {
    fn status(&self) -> harmos_domain::operations::instance::Status {
        self.status().portable()
    }
    fn cancel(&self) {
        self.cancel();
    }
    fn subscribe(&self) -> Box<dyn harmos_sdk::operations::instances::InstanceChanges> {
        Box::new(InstanceChanges(self.settlement()))
    }
}
struct InstanceChanges(tokio::sync::watch::Receiver<super::OperationStatus>);
impl harmos_sdk::operations::instances::InstanceChanges for InstanceChanges {
    fn current(&mut self) -> harmos_domain::operations::instance::Status {
        self.0.borrow_and_update().portable()
    }
    fn changed(&mut self) -> harmos_sdk::operations::instances::Changed<'_> {
        Box::pin(async { self.0.changed().await.is_ok() })
    }
}

impl harmos_sdk::operations::access::Discovery for HostContext {
    fn discover(
        &self, after: Option<&str>, rows: usize, bytes: usize,
    ) -> std::result::Result<harmos_sdk::operations::access::DiscoveryPage, harmos_sdk::operations::access::AccessError> {
        let context = self.borrow();
        let request = harmos_operations::host::CatalogRequest {
            kind: harmos_operations::host::CatalogKind::Operations,
            schema: None,
            after: after.map(str::to_owned),
            rows: Some(rows),
            bytes: Some(bytes),
        };
        let page = context.operations.discover(&context.source, &request)?;
        Ok((page.entries, page.next))
    }
}
impl harmos_sdk::operations::access::GrantAuthority for HostContext {
    fn admit(&self, grant: &harmos_domain::plugin::Grant) -> std::result::Result<(), harmos_sdk::operations::access::AccessError> {
        self.borrow().admit_stream_grant(grant).map_err(Into::into)
    }
}

#[cfg(feature = "plugin-sidecar")]
impl harmos_sdk::operations::publication::PublicationHost for HostContext {
    fn publish<'a>(
        &'a self, _: harmos_sdk::OperationContext, publication: harmos_sdk::operations::publication::Publication<'a>,
    ) -> harmos_sdk::operations::publication::Published<'a> {
        Box::pin(async move {
            let context = self.borrow();
            let scope = context.caller_scope().ok_or(Error::<std::convert::Infallible>::InvalidInput {
                field: "native publication",
                reason: "the original plugin caller is required",
            })?;
            let endpoint = scope.native().ok_or(Error::<std::convert::Infallible>::NotReady { domain: "native publication endpoint" })?;
            endpoint.publish(scope, publication, &context).await.map_err(Into::into)
        })
    }
}
