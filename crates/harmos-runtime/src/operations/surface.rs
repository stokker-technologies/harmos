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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[must_use = "an invocation runs only when start or run is awaited"]
pub struct Invocation<O: OperationDeclaration, L: Lane, E: EndpointSet = ()> {
    target: Target,
    id: Option<String>,
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
        Self {
            target: Target::Host(operations),
            id: None,
            request,
            retry: None,
            lane: None,
            endpoints: (),
        }
    }
}

impl<O: OperationDeclaration, L: Lane, E: EndpointSet> Invocation<O, L, E> {
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

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
            target: self.target,
            id: self.id,
            request: self.request,
            retry: self.retry,
            lane: self.lane,
            endpoints,
        }
    }

    pub async fn start(self) -> Result<Running<O>> {
        if E::item_types() != super::item_types::<O>() {
            return Err(Error::InvalidInput {
                field: "operation endpoints",
                reason: "endpoint items differ from the declaration",
            });
        }
        let mut request = super::binding::Request::from(self.request);
        request.retry = self.retry;
        request.lane = self.lane.map(|lane| lane.name());
        let (operations, source) = match self.target {
            Target::Host(operations) => (operations, Source::Host),
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            Target::Plugin(scope) => (scope.operation_manager()?, Source::Scope(scope)),
        };
        let id = self.id.as_deref().unwrap_or(O::ID);
        operations.admit_at::<O, E>(id, (request, self.endpoints), None, &[], &source, None)
    }

    pub async fn run(self) -> Result<Response<O::Response>> {
        self.start().await?.join().await
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl crate::plugin::PluginScope {
    pub fn operation<O: OperationDeclaration<Request = O>, L: Lane>(&self, request: O) -> Invocation<O, L> {
        Invocation {
            target: Target::Plugin(self.clone()),
            id: None,
            request,
            retry: None,
            lane: None,
            endpoints: (),
        }
    }

    pub async fn register<C: harmos_sdk::operations::OperationCatalog, L: Lane>(&self, lane: L) -> Result<()> {
        use super::RegistrationExt;
        self.register_batch(C::batch().lane(lane)).await
    }

    pub async fn register_batch<L: Lane>(&self, mut registration: super::Registration<L>) -> Result<()> {
        if !self.is_ready() {
            return Err(Error::Closing { domain: "plugin scope" });
        }
        let operations = self.operation_manager()?;
        let services = std::mem::take(&mut registration.batch.services);
        if services.is_empty() {
            let entries = operations.native_entries(registration, self.owner(), Some(self))?;
            return operations.reserve(&entries);
        }
        let mut batch = harmos_sdk::__private::RegistrationBatch::new();
        batch.services = services;
        let manager = crate::ServiceManager::scoped(operations.clone(), self.clone());
        let (staged, ids) = manager.stage(batch)?;
        operations.register_scoped_services(registration, staged, self.owner(), Some(self))?;
        manager.activate(&ids).await
    }

    pub async fn unregister(&self, id: &harmos_domain::QualifiedId) -> Result<()> {
        let (owner, _) = id.as_str().split_once('.').expect("validated qualified identity");
        if owner != self.owner() {
            return Err(Error::PermissionDenied);
        }
        let operations = self.operation_manager()?;
        if matches!(operations.resident.definition(id.as_str()), Some(super::resident::Definition::Service(_))) {
            return crate::ServiceManager::scoped(operations, self.clone()).unregister(id).await;
        }
        operations.unregister(id).await
    }
}

impl<A: crate::Application> crate::Runtime<A> {
    pub fn services(&self) -> crate::ServiceManager {
        crate::ServiceManager::new(self.operations.execution())
    }

    pub async fn register<C: harmos_sdk::operations::OperationCatalog>(&self, lane: A::Lanes) -> Result<()> {
        use super::RegistrationExt;
        self.register_batch(C::batch().lane(lane)).await
    }

    pub async fn register_batch(&self, mut registration: super::Registration<A::Lanes>) -> Result<()> {
        let services = std::mem::take(&mut registration.batch.services);
        if services.is_empty() {
            return self.operations.register_batch(registration).await;
        }
        let mut batch = harmos_sdk::__private::RegistrationBatch::new();
        batch.services = services;
        let manager = self.services();
        let (staged, ids) = manager.stage(batch)?;
        self.operations.execution().register_with_services(registration, staged)?;
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

    pub fn operation<O: OperationDeclaration<Request = O>>(&self, request: O) -> Invocation<O, A::Lanes> {
        Invocation::new(self.operations.execution(), request)
    }
}

impl<A: crate::Application> crate::runtime::Handle<A> {
    pub fn services(&self) -> crate::ServiceManager {
        crate::ServiceManager::new(self.operations.execution())
    }

    pub async fn register<C: harmos_sdk::operations::OperationCatalog>(&self, lane: A::Lanes) -> Result<()> {
        use super::RegistrationExt;
        self.register_batch(C::batch().lane(lane)).await
    }

    pub async fn register_batch(&self, mut registration: super::Registration<A::Lanes>) -> Result<()> {
        let services = std::mem::take(&mut registration.batch.services);
        if services.is_empty() {
            return self.operations.register_batch(registration).await;
        }
        let mut batch = harmos_sdk::__private::RegistrationBatch::new();
        batch.services = services;
        let manager = self.services();
        let (staged, ids) = manager.stage(batch)?;
        self.operations.execution().register_with_services(registration, staged)?;
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

    pub fn operation<O: OperationDeclaration<Request = O>>(&self, request: O) -> Invocation<O, A::Lanes> {
        Invocation::new(self.operations.execution(), request)
    }
}

impl<L: Lane> harmos_sdk::operations::Host for super::OperationManager<L> {
    fn publish_on<S: harmos_domain::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        use harmos_domain::Message;
        let published = self.call::<harmos_operations::streams::Publish>(harmos_operations::streams::Publish {
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

    fn submit_owned<O: OperationDeclaration>(
        &self, request: O::Request, claims: super::RequestClaims,
    ) -> impl Future<Output = Result<Running<O>>> + Send {
        std::future::ready(Manager::submit_owned::<O>(self, request, claims))
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = Manager::submit::<O>(self, request);
        async move { submitted?.await }
    }
}

impl harmos_sdk::operations::Host for Manager {
    fn publish_on<S: harmos_domain::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        use harmos_domain::Message;
        let published = self.call::<harmos_operations::streams::Publish>(harmos_operations::streams::Publish {
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

    fn submit_owned<O: OperationDeclaration>(
        &self, request: O::Request, claims: super::RequestClaims,
    ) -> impl Future<Output = Result<Running<O>>> + Send {
        std::future::ready(Manager::submit_owned::<O>(self, request, claims))
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        let submitted = Manager::submit::<O>(self, request);
        async move { submitted?.await }
    }
}

impl harmos_sdk::operations::Host for Context<'_> {
    fn publish_on<S: harmos_domain::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        use harmos_domain::Message;
        let published = self.call::<harmos_operations::streams::Publish>(harmos_operations::streams::Publish {
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

    fn submit_owned<O: OperationDeclaration>(
        &self, request: O::Request, claims: super::RequestClaims,
    ) -> impl Future<Output = Result<Running<O>>> + Send {
        std::future::ready(Context::submit_owned::<O>(self, request, claims))
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
    use crate::Application;
    use crate::operations::RegistrationExt;
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
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }

        async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
            Ok(())
        }

        async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Application for Model {
        type Lanes = Lanes;
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
        type Streams = ();
    }

    #[crate::operation(harmos_path = crate, resources = [AtomicUsize, crate::operations::HostContext],
        id = "test.probe", response = u64,
        policy = crate::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
    struct Probe {
        value: u64,
    }

    impl crate::Operation for Probe {
        type Error = std::io::Error;

        async fn run(self, ctx: crate::Context<'_, Self>) -> std::io::Result<Response<u64>> {
            let host = ctx.resource::<crate::operations::HostContext>();
            let attempt = host.borrow().attempt();
            if attempt == 0 {
                ctx.resource::<AtomicUsize>().fetch_add(1, Ordering::SeqCst);
            }
            if u64::try_from(attempt).unwrap() < self.value {
                return Err(std::io::Error::other("retry this request"));
            }
            Ok(0.into())
        }

        fn classify(_: &std::io::Error) -> super::super::FailureKind {
            super::super::FailureKind::Transient
        }
    }

    struct Idle;

    impl harmos_sdk::Service for Idle {
        type Error = std::io::Error;
        async fn initialize(_: harmos_sdk::ExecutionContext) -> std::io::Result<Self> {
            Ok(Self)
        }
        async fn run(&mut self, _: harmos_sdk::ExecutionContext) -> std::io::Result<()> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn mixed_registration_releases_invalid_batch_reservations() {
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource::<AtomicUsize>(Arc::new(AtomicUsize::new(0)));
        })
        .await
        .unwrap();
        let batch = harmos_sdk::__private::RegistrationBatch::new().operation::<Probe>().service::<Idle>(
            "idle",
            crate::ServicePolicy {
                stop_timeout: std::time::Duration::from_secs(1),
                restart: crate::RestartPolicy::Never,
            },
        );
        assert!(
            runtime
                .register_batch(batch.clone().operation::<Probe>().lane(<Lanes as crate::operations::Lane>::default_lane()))
                .await
                .is_err()
        );
        assert!(runtime.operations.definition(Probe::ID).is_err());
        assert!(runtime.operations.resident.definition("host.idle").is_none());
        runtime.register_batch(batch.lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
        assert!(runtime.operations.definition(Probe::ID).is_ok());
        assert!(runtime.operations.resident.definition("host.idle").is_some());
        runtime.unregister(&crate::QualifiedId::parse("host.idle").unwrap()).await.unwrap();
        runtime.unregister(&crate::QualifiedId::parse("host.test.probe").unwrap()).await.unwrap();
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn operation_and_service_share_one_identity_namespace() {
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource::<AtomicUsize>(Arc::new(AtomicUsize::new(0)));
        })
        .await
        .unwrap();
        let operation = harmos_sdk::__private::RegistrationBatch::new().operation::<Probe>();
        let policy = crate::ServicePolicy {
            stop_timeout: std::time::Duration::from_secs(1),
            restart: crate::RestartPolicy::Never,
        };
        let service = harmos_sdk::__private::RegistrationBatch::new().service::<Idle>("test.probe", policy);
        assert!(
            runtime
                .register_batch(
                    operation.clone().service::<Idle>("test.probe", policy).lane(<Lanes as crate::operations::Lane>::default_lane())
                )
                .await
                .is_err()
        );
        assert!(runtime.operations.definition(Probe::ID).is_err());
        assert!(!runtime.operations.resident.contains("host.test.probe"));
        runtime.operations.register_batch(operation.clone().lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
        assert!(runtime.services().register_batch(service.clone()).await.is_err());
        runtime.unregister(&crate::QualifiedId::parse("host.test.probe").unwrap()).await.unwrap();
        runtime.services().register_batch(service).await.unwrap();
        assert!(runtime.operations.register_batch(operation.lane(<Lanes as crate::operations::Lane>::default_lane())).await.is_err());
        runtime.unregister(&crate::QualifiedId::parse("host.test.probe").unwrap()).await.unwrap();
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn native_registration_is_atomic_and_invocation_policy_is_local() {
        let initialized = Arc::new(AtomicUsize::new(0));
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource::<AtomicUsize>(initialized.clone());
        })
        .await
        .unwrap();
        let batch = harmos_sdk::__private::RegistrationBatch::new().operation::<Probe>();
        assert!(
            runtime
                .register_batch(batch.clone().operation::<Probe>().lane(<Lanes as crate::operations::Lane>::default_lane()))
                .await
                .is_err()
        );
        assert!(runtime.operations.definition(Probe::ID).is_err());
        let mut mismatched = batch.clone();
        mismatched.operations[0].definition.id = crate::Identifier::new("different").unwrap();
        assert!(runtime.register_batch(mismatched.lane(<Lanes as crate::operations::Lane>::default_lane())).await.is_err());
        assert!(runtime.operations.definition("different").is_err());
        runtime.operations.register_batch(batch.lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
        assert!(matches!(
            runtime.operations.call_encoded("host.test.probe", Vec::new()).await,
            Err(Error::InvalidInput { field: "operation preparation bytes", .. })
        ));
        let invocation = runtime.operation(Probe { value: 1 });
        assert_eq!(initialized.load(Ordering::SeqCst), 0);
        assert!(invocation.run().await.is_err());
        assert_eq!(initialized.load(Ordering::SeqCst), 1);

        let retry = RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: crate::Backoff::Fixed {
                delay: std::time::Duration::from_millis(1),
                jitter: crate::Jitter::None,
            },
        };
        assert_eq!(*runtime.operation(Probe { value: 1 }).retry(retry).run().await.unwrap(), 0);
        assert_eq!(initialized.load(Ordering::SeqCst), 2);
        assert!(runtime.operation(Probe { value: 1 }).run().await.is_err());
        assert_eq!(initialized.load(Ordering::SeqCst), 3);
        assert_eq!(runtime.operations.definition(Probe::ID).unwrap().policy.retries, RetryPolicy::Never);
        runtime.operations.unregister(&crate::QualifiedId::parse("host.test.probe").unwrap()).await.unwrap();
        assert!(runtime.operation(Probe { value: 0 }).start().await.is_err());
        runtime.shutdown().await.unwrap();
    }
}
