/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// In-process binding of the portable host: a thin layer over the
// manager's own submission, so in-process and remote bodies share one shape.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;

// Third Party Imports

// Local Imports
use super::caller::Source;
use super::{CallContext, Context, ContextError, ContextValue, OperationDeclaration, OperationManager, Response, Running};
use super::{EndpointSet, Lane, RetryPolicy};
use crate::{Error, Result};
use harmos_domain::RequestFor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "an invocation runs only when start or run is awaited"]
pub struct Invocation<O: OperationDeclaration, L: Lane, E: EndpointSet = ()> {
    operations: OperationManager,
    request: O::Request,
    retry: Option<RetryPolicy>,
    lane: Option<L>,
    endpoints: E,
    source: Source,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<O: OperationDeclaration, L: Lane> Invocation<O, L> {
    pub(crate) fn new(operations: OperationManager, request: O::Request) -> Self {
        Self { operations, request, retry: None, lane: None, endpoints: (), source: Source::Host }
    }
}

impl<O: OperationDeclaration, L: Lane, E: EndpointSet> Invocation<O, L, E> {
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    pub fn lane(mut self, lane: L) -> Self {
        self.lane = Some(lane);
        self
    }

    pub fn endpoints<F: EndpointSet>(self, endpoints: F) -> Invocation<O, L, F> {
        Invocation {
            operations: self.operations,
            request: self.request,
            retry: self.retry,
            lane: self.lane,
            endpoints,
            source: self.source,
        }
    }

    pub async fn start(self) -> Result<Running<O>> {
        if E::item_types() != super::item_types::<O>() {
            return Err(Error::InvalidInput { field: "operation endpoints", reason: "endpoint items differ from the declaration" });
        }
        let mut request = super::binding::Request::from(self.request);
        request.retry = self.retry;
        request.lane = self.lane.map(|lane| lane.name());
        self.operations.admit::<O, E>((request, self.endpoints), None, &[], &self.source, None)
    }

    pub async fn run(self) -> Result<Response<O::Response>> {
        self.start().await?.join().await
    }
}

impl<A: crate::Application> crate::Runtime<A> {
    pub fn services(&self) -> crate::ServiceManager {
        crate::ServiceManager::for_application::<A>(self.operations.clone())
    }

    pub async fn register(&self, mut registration: super::Registration<A::Lanes>) -> Result<()> {
        let services = std::mem::take(&mut registration.batch.services);
        if services.is_empty() {
            return self.operations.register(registration).await;
        }
        let mut batch = harmos_sdk::RegistrationBatch::new(registration.batch.dependencies.clone());
        batch.services = services;
        let manager = self.services();
        let (staged, ids) = manager.stage(batch)?;
        self.operations.register(registration).await?;
        staged.commit()?;
        manager.activate(&ids).await
    }

    pub async fn unregister(&self, id: &harmos_domain::QualifiedId) -> Result<()> {
        if !id.as_str().starts_with("host.") {
            return Err(Error::PermissionDenied);
        }
        if matches!(self.operations.resident.definition(id.as_str()), Some(super::resident::Definition::Service(_))) {
            return self.services().unregister(id).await;
        }
        self.operations.unregister(id).await
    }

    pub fn operation<R: RequestFor>(&self, request: R) -> Invocation<R::Operation, A::Lanes> {
        self.operation_for::<R::Operation>(request)
    }

    pub fn operation_for<O: OperationDeclaration>(&self, request: O::Request) -> Invocation<O, A::Lanes> {
        Invocation::new(self.operations.clone(), request)
    }
}

impl<A: crate::Application> crate::runtime::Handle<A> {
    pub fn services(&self) -> crate::ServiceManager {
        crate::ServiceManager::for_application::<A>(self.operations.clone())
    }

    pub async fn register(&self, mut registration: super::Registration<A::Lanes>) -> Result<()> {
        let services = std::mem::take(&mut registration.batch.services);
        if services.is_empty() {
            return self.operations.register(registration).await;
        }
        let mut batch = harmos_sdk::RegistrationBatch::new(registration.batch.dependencies.clone());
        batch.services = services;
        let manager = self.services();
        let (staged, ids) = manager.stage(batch)?;
        self.operations.register(registration).await?;
        staged.commit()?;
        manager.activate(&ids).await
    }

    pub async fn unregister(&self, id: &harmos_domain::QualifiedId) -> Result<()> {
        if !id.as_str().starts_with("host.") {
            return Err(Error::PermissionDenied);
        }
        if matches!(self.operations.resident.definition(id.as_str()), Some(super::resident::Definition::Service(_))) {
            return self.services().unregister(id).await;
        }
        self.operations.unregister(id).await
    }

    pub fn operation<R: RequestFor>(&self, request: R) -> Invocation<R::Operation, A::Lanes> {
        self.operation_for::<R::Operation>(request)
    }

    pub fn operation_for<O: OperationDeclaration>(&self, request: O::Request) -> Invocation<O, A::Lanes> {
        Invocation::new(self.operations.clone(), request)
    }
}

impl harmos_sdk::operations::Host for OperationManager {
    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;

    fn context(&self) -> &CallContext {
        self.call_context()
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let context = self.call_context().with_value(value)?;
        Ok(self.with_context(context))
    }

    fn submit<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Running<O>>> + Send {
        std::future::ready(OperationManager::submit::<O>(self, request))
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = OperationManager::submit::<O>(self, request);
        async move { submitted?.await }
    }
}

impl harmos_sdk::operations::Host for Context<'_> {
    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;

    fn context(&self) -> &CallContext {
        Context::context(self)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let context = Context::context(self).with_value(value)?;
        let mut child = self.clone();
        child.operations = child.operations.with_context(context);
        Ok(child)
    }

    fn submit<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Running<O>>> + Send {
        std::future::ready(Context::submit::<O>(self, request))
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = Context::submit::<O>(self, request);
        async move { submitted?.await }
    }

    // A resident outlives the attempt that activated it; its lease names the
    // attempt's caller.
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
