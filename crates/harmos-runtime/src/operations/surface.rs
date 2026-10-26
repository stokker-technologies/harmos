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
use super::{CallContext, Context, ContextError, ContextValue, Manager, OperationDeclaration, Response, Running};
use super::{EndpointSet, Lane, RetryPolicy};
use crate::{Error, Result};
use harmos_domain::RequestFor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "an invocation runs only when start or run is awaited"]
pub struct Invocation<O: OperationDeclaration, L: Lane, E: EndpointSet = ()> {
    target: Target,
    request: O::Request,
    retry: Option<RetryPolicy>,
    lane: Option<L>,
    endpoints: E,
}

/* === Enums === */

enum Target {
    Host(Manager),
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    Plugin(crate::plugin::PluginScope),
}

/* === Traits === */

/* === Implementations === */

impl<O: OperationDeclaration, L: Lane> Invocation<O, L> {
    pub(crate) fn new(operations: Manager, request: O::Request) -> Self {
        Self { target: Target::Host(operations), request, retry: None, lane: None, endpoints: () }
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
        Invocation { target: self.target, request: self.request, retry: self.retry, lane: self.lane, endpoints }
    }

    pub async fn start(self) -> Result<Running<O>> {
        if E::item_types() != super::item_types::<O>() {
            return Err(Error::InvalidInput { field: "operation endpoints", reason: "endpoint items differ from the declaration" });
        }
        let mut request = super::binding::Request::from(self.request);
        request.retry = self.retry;
        request.lane = self.lane.map(|lane| lane.name());
        let (operations, source) = match self.target {
            Target::Host(operations) => (operations, Source::Host),
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            Target::Plugin(scope) => (scope.operation_manager()?, Source::Scope(scope)),
        };
        operations.admit::<O, E>((request, self.endpoints), None, &[], &source, None)
    }

    pub async fn run(self) -> Result<Response<O::Response>> {
        self.start().await?.join().await
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl crate::plugin::PluginScope {
    pub fn operation<R: RequestFor, L: Lane>(&self, request: R) -> Invocation<R::Operation, L> {
        self.operation_for::<R::Operation, L>(request)
    }

    pub fn operation_for<O: OperationDeclaration, L: Lane>(&self, request: O::Request) -> Invocation<O, L> {
        Invocation { target: Target::Plugin(self.clone()), request, retry: None, lane: None, endpoints: () }
    }

    pub async fn register<L: Lane>(&self, registration: super::Registration<L>) -> Result<()> {
        if !self.is_ready() {
            return Err(Error::Closing { domain: "plugin scope" });
        }
        let operations = self.operation_manager()?;
        let entries = operations.native_entries(registration, self.owner(), Some(self))?;
        operations.reserve(&entries)
    }

    pub async fn unregister(&self, id: &harmos_domain::QualifiedId) -> Result<()> {
        let (owner, _) = id.as_str().split_once('.').expect("validated qualified identity");
        if owner != self.owner() {
            return Err(Error::PermissionDenied);
        }
        self.operation_manager()?.unregister(id).await
    }
}

impl<A: crate::Application> crate::Runtime<A> {
    pub fn services(&self) -> crate::ServiceManager {
        crate::ServiceManager::for_application::<A>(self.operations.execution())
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
        Invocation::new(self.operations.execution(), request)
    }
}

impl<A: crate::Application> crate::runtime::Handle<A> {
    pub fn services(&self) -> crate::ServiceManager {
        crate::ServiceManager::for_application::<A>(self.operations.execution())
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
        Invocation::new(self.operations.execution(), request)
    }
}

impl<L: Lane> harmos_sdk::operations::Host for super::OperationManager<L> {
    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;

    fn context(&self) -> &CallContext {
        self.call_context()
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let context = self.call_context().with_value(value)?;
        Ok(Self::new(self.execution().with_context(context)))
    }

    fn submit<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Running<O>>> + Send {
        std::future::ready(Manager::submit::<O>(self, request))
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = Manager::submit::<O>(self, request);
        async move { submitted?.await }
    }
}

impl harmos_sdk::operations::Host for Manager {
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
        std::future::ready(Manager::submit::<O>(self, request))
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = Manager::submit::<O>(self, request);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Application, RegistrationExt};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[crate::lanes]
    enum Lanes {
        #[default]
        Default,
    }

    struct Model;
    impl Application for Model {
        type Lanes = Lanes;
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
        type Streams = ();
        type Resources = ();
    }

    struct Probe;
    impl OperationDeclaration for Probe {
        type Request = u64;
        type Response = u64;
        type Input = super::super::NoStream;
        type Output = super::super::NoStream;
        const ID: &'static str = "probe";
        const POLICY: crate::OperationPolicy = crate::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            retries: RetryPolicy::Never,
        };
    }

    impl harmos_sdk::Operation for Probe {
        type Request = u64;
        type Response = u64;
        type Error = std::io::Error;
        type Endpoints = ();

        async fn initialize(context: harmos_sdk::OperationContext) -> std::io::Result<Self> {
            context.dependency::<AtomicUsize>().map_err(std::io::Error::other)?.fetch_add(1, Ordering::SeqCst);
            Ok(Self)
        }

        async fn run(&mut self, _: harmos_sdk::OperationContext, request: &mut u64, _: &mut ()) -> std::io::Result<Response<u64>> {
            if *request != 0 {
                *request -= 1;
                return Err(std::io::Error::other("retry this request"));
            }
            Ok((*request).into())
        }

        fn classify(_: &std::io::Error) -> super::super::FailureKind {
            super::super::FailureKind::Transient
        }
    }

    #[tokio::test]
    async fn native_registration_is_atomic_and_invocation_policy_is_local() {
        let runtime = crate::Runtime::<Model>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
        let initialized = Arc::new(AtomicUsize::new(0));
        let mut dependencies = harmos_sdk::Dependencies::new();
        dependencies.insert(initialized.clone()).unwrap();
        let batch = harmos_sdk::RegistrationBatch::new(dependencies).operation::<Probe>();
        assert!(runtime.register(batch.clone().operation::<Probe>().defaults()).await.is_err());
        assert!(runtime.operations.definition(Probe::ID).is_err());
        let mut mismatched = batch.clone();
        mismatched.operations[0].definition.id = crate::Identifier::new("different").unwrap();
        assert!(runtime.register(mismatched.defaults()).await.is_err());
        assert!(runtime.operations.definition("different").is_err());
        runtime.operations.register(batch.defaults()).await.unwrap();
        assert!(matches!(
            runtime.operations.call_encoded("host.probe", Vec::new()).await,
            Err(Error::InvalidInput { field: "operation preparation bytes", .. })
        ));
        let invocation = runtime.operation_for::<Probe>(1);
        assert_eq!(initialized.load(Ordering::SeqCst), 0);
        assert!(invocation.run().await.is_err());
        assert_eq!(initialized.load(Ordering::SeqCst), 1);

        let retry = RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: crate::Backoff::Fixed { delay: std::time::Duration::from_millis(1), jitter: crate::Jitter::None },
        };
        assert_eq!(*runtime.operation_for::<Probe>(1).retry(retry).run().await.unwrap(), 0);
        assert_eq!(initialized.load(Ordering::SeqCst), 2);
        assert!(runtime.operation_for::<Probe>(1).run().await.is_err());
        assert_eq!(initialized.load(Ordering::SeqCst), 3);
        assert_eq!(runtime.operations.definition(Probe::ID).unwrap().policy.retries, RetryPolicy::Never);
        runtime.operations.unregister(&crate::QualifiedId::parse("host.probe").unwrap()).await.unwrap();
        assert!(runtime.operation_for::<Probe>(0).start().await.is_err());
        runtime.stop().await.unwrap();
    }
}
