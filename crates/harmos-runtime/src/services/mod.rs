/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resident Services Use Independent Restart Credit And Bounded Stop Deadlines.

/* === Modules === */

mod restart;
mod runner;
pub mod status;

/* === Re-Exports === */

pub(crate) use runner::{Once, Typed};

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use crate::ServicePolicy;
use crate::operations::{Backoff, CallContext, ContextError, ContextValue, Jitter, Manager, RequestShape, RestartPolicy, Running};
impl<A: Application> harmos_operations::execution::operations::ProtobufHost for ServiceContext<A> {
    type Error = Error;
    async fn call_protobuf<O: crate::operations::ProtobufOperation<Endpoints = ()>>(&self, request: O::Request) -> Result<O::Response> {
        harmos_operations::execution::operations::ProtobufHost::call_protobuf::<O>(&self.operations, request).await
    }
}

#[cfg(test)]
use crate::supervision::executor;
use crate::{Application, Error, Result, state::StateManager, streams::StreamManager};

/* === Constants === */

// Three Restart Credits Refill After Sustained Health.
pub const POLICY: ServicePolicy = ServicePolicy {
    stop_timeout: Duration::from_secs(5),
    restart: RestartPolicy::OnFailure {
        credit: 3,
        sustained_health: Duration::from_millis(250),
        backoff: Backoff::Exponential {
            initial: Duration::from_millis(10),
            maximum: Duration::from_secs(5),
            jitter: Jitter::Deterministic,
        },
    },
};

/* === Macros === */

/* === Type Aliases === */

pub(crate) type ServiceRunner = fn(
    ServiceDefinition,
    harmos_operations::execution::operations::ServiceFactory,
    ServiceOwner,
) -> Arc<dyn crate::operations::resident::Runner>;

#[derive(Clone)]
pub(crate) enum ServiceOwner {
    Host,

    #[cfg(feature = "plugin-guest")]
    Plugin(crate::guest::host::plugin::PluginScope),
}

pub(crate) fn authored_runner<A: Application>(
    definition: ServiceDefinition, factory: harmos_operations::execution::operations::ServiceFactory, owner: ServiceOwner,
) -> Arc<dyn crate::operations::resident::Runner> {
    Arc::new(Typed::<A, harmos_operations::execution::ExecutionContext, _>::new(definition, move |_| AuthoredService {
        factory: factory.clone(),
        instance: None,
        owner: owner.clone(),
    }))
}

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ServiceDefinition {
    pub id: String,
    pub policy: ServicePolicy,
}

// The runtime managers a service context lends as typed values.
pub(crate) struct Handles<A: Application> {
    pub(crate) storage: Arc<std::sync::OnceLock<crate::storage::StorageManager>>,
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::records::RecordManager<A>,
    pub(crate) streams: StreamManager<A>,
}

#[derive(Clone)]
pub struct ServiceManager {
    operations: Manager,
    owner: ServiceOwner,
}

struct AuthoredService {
    owner: ServiceOwner,
    factory: harmos_operations::execution::operations::ServiceFactory,
    instance: Option<Box<dyn harmos_operations::execution::operations::ServiceInstance>>,
}

pub struct ServiceContext<A: Application> {
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::records::RecordManager<A>,
    pub(crate) operations: Manager,
    pub(crate) streams: StreamManager<A>,
    pub(crate) stopping: watch::Receiver<bool>,
    pub(crate) ready: watch::Sender<bool>,
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServiceExit {
    #[error("application: {0}")]
    Application(String),

    #[error("resident panicked")]
    Panicked,

    #[error("restart credit exhausted")]
    CreditExhausted,

    #[error("resident stop deadline exceeded")]
    StopTimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceStatus {
    Starting,
    Running { credit: usize },
    Stopping,
    BackingOff,
    Stopped,
    Inactive { cause: ServiceExit, history: Vec<String> },
}

/* === Traits === */

// A resident runs until stopped. Each run is stopped and cleaned up before the
// next restart or before the instance stops.
pub trait Resident<A: Application>: Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn run(&mut self, context: ServiceContext<A>) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send;

    fn stop(&mut self) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn cleanup(&mut self) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send;
}

/* === Implementations === */

impl ServiceManager {
    pub(crate) fn new(operations: Manager) -> Self {
        Self { operations, owner: ServiceOwner::Host }
    }
    #[cfg(feature = "plugin-guest")]
    pub(crate) fn scoped(operations: Manager, scope: crate::guest::host::plugin::PluginScope) -> Self {
        Self {
            operations,
            owner: ServiceOwner::Plugin(scope),
        }
    }
    pub fn configure_status(&self, settings: status::StatusSettings) -> Result<Arc<status::Status>> {
        if self.owner() != "host" {
            return Err(Error::PermissionDenied);
        }
        let mut owner = self.operations.registry.status_owner.lock().expect("status owner lock is healthy");
        if owner.is_some() {
            return Err(Error::AlreadyRegistered {
                domain: "host status",
                name: "Status".into(),
            });
        }
        let status =
            status::Status::new(settings, self.operations.clock().clone(), self.operations.registry.status_started).map_err(|source| {
                Error::OperationFailed {
                    operation: "Status.Configure".into(),
                    source: Arc::new(source),
                }
            })?;
        status.refresh().map_err(|source| Error::OperationFailed {
            operation: "Status.Configure".into(),
            source: Arc::new(source),
        })?;
        let catalog = harmos_operations::status::catalog().map_err(|source| Error::OperationFailed {
            operation: "Status.Configure".into(),
            source: Arc::new(source),
        })?;
        use crate::operations::RegistrationExt;
        self.operations.register_native(catalog.lane(crate::operations::Lane::default()))?;
        self.operations.registry.status.0.set(Arc::new(status::Port(status.clone()))).map_err(|_| Error::AlreadyRegistered {
            domain: "host status",
            name: "Status".into(),
        })?;
        *owner = Some(status.clone());
        Ok(status)
    }
    pub fn status(&self) -> Result<Arc<status::Status>> {
        self.operations
            .registry
            .status_owner
            .lock()
            .expect("status owner lock is healthy")
            .clone()
            .ok_or(Error::NotReady { domain: "Status" })
    }
    fn owner(&self) -> &str {
        match &self.owner {
            ServiceOwner::Host => "host",
            #[cfg(feature = "plugin-guest")]
            ServiceOwner::Plugin(scope) => scope.owner(),
        }
    }

    pub async fn register<S: harmos_operations::execution::Service>(&self, id: &str, policy: crate::ServicePolicy) -> Result<()> {
        let batch = harmos_operations::execution::__private::Catalog::new().service::<S>(id, policy);
        self.register_batch(batch).await
    }

    pub(crate) async fn register_batch(&self, batch: harmos_operations::execution::__private::Catalog) -> Result<()> {
        let (staged, ids) = self.stage(batch)?;
        staged.commit()?;
        self.activate(&ids).await
    }

    pub(crate) fn stage(
        &self, batch: harmos_operations::execution::__private::Catalog,
    ) -> Result<(crate::operations::resident::Staged, Vec<String>)> {
        if !batch.operations.is_empty() {
            return Err(Error::InvalidInput {
                field: "service registration",
                reason: "operations belong to the operation manager",
            });
        }
        let mut entries = Vec::with_capacity(batch.services.len());
        let mut ids = Vec::with_capacity(batch.services.len());
        for factory in batch.services {
            let id = crate::QualifiedId::qualify(self.owner(), &factory.local_id).map_err(|_| Error::InvalidInput {
                field: "service identity",
                reason: "invalid local service identifier",
            })?;
            let definition = ServiceDefinition {
                id: id.as_str().to_owned(),
                policy: factory.policy,
            };
            validate(&definition)?;
            ids.push(definition.id.clone());
            let runner = (self.operations.registry.service_runner)(definition.clone(), factory, self.owner.clone());
            entries.push((crate::operations::resident::Definition::Service(definition), runner));
        }
        let staged = self.operations.stage_services(
            entries,
            #[cfg(feature = "plugin-guest")]
            match &self.owner {
                ServiceOwner::Host => None,
                ServiceOwner::Plugin(scope) => Some(scope.clone()),
            },
        )?;
        Ok((staged, ids))
    }

    pub(crate) async fn activate(&self, ids: &[String]) -> Result<()> {
        for id in ids {
            if let Err(error) = self.operations.start_registered_service(id, &self.owner).await {
                for id in ids {
                    self.operations.resident.unregister(id).await?;
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub async fn unregister(&self, id: &crate::QualifiedId) -> Result<()> {
        if id.as_str().split_once('.').map(|(owner, _)| owner) != Some(self.owner()) {
            return Err(Error::PermissionDenied);
        }
        self.operations.resident.unregister(id.as_str()).await
    }
}

impl<A: Application> Resident<A> for AuthoredService {
    type Error = harmos_operations::execution::ServingError;

    async fn run(&mut self, context: ServiceContext<A>) -> std::result::Result<(), Self::Error> {
        let mut stopping = context.stopping.clone();
        let context = Arc::new(context);
        let mut dependencies = harmos_operations::execution::__private::ResourceSet::new();
        match &self.owner {
            ServiceOwner::Host => dependencies.insert(context.clone()).map_err(|_| harmos_operations::execution::ServingError::Catalog)?,
            #[cfg(feature = "plugin-guest")]
            ServiceOwner::Plugin(scope) => {
                dependencies.insert(Arc::new(scope.clone())).map_err(|_| harmos_operations::execution::ServingError::Catalog)?
            }
        }
        let call = harmos_operations::execution::ExecutionContext::new(
            dependencies,
            harmos_operations::execution::operations::OperationHost::context(&context.operations).clone(),
        )
        .with_resources(context.operations.resources().clone());
        self.instance = Some(self.factory.initialize(call.clone()).await?);
        context.ready();
        tokio::select! {
            biased;
            _ = stopping.wait_for(|stopping| *stopping) => Ok(()),
            result = self.instance.as_mut().expect("initialized service").run(call) => result,
        }
    }

    async fn stop(&mut self) -> std::result::Result<(), Self::Error> {
        if let Some(instance) = &mut self.instance {
            instance.stop().await?;
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        if let Some(instance) = &mut self.instance {
            instance.cleanup().await?;
        }
        self.instance = None;
        Ok(())
    }
}

impl ServiceDefinition {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), policy: POLICY }
    }
}

impl<A: Application> Handles<A> {
    pub(crate) fn context(&self, operations: Manager, stopping: watch::Receiver<bool>, ready: watch::Sender<bool>) -> ServiceContext<A> {
        ServiceContext {
            state: self.state.clone(),
            records: self.records.clone(),
            operations,
            streams: self.streams.clone(),
            stopping,
            ready,
        }
    }
}

impl<A: Application> Clone for Handles<A> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            state: self.state.clone(),
            records: self.records.clone(),
            streams: self.streams.clone(),
        }
    }
}

impl<A: Application> ServiceContext<A> {
    pub fn resource<T: Send + Sync + 'static>(&self) -> std::result::Result<Arc<T>, harmos_operations::execution::ResourceError> {
        self.operations.resources().resource::<T>()
    }

    pub fn ready(&self) {
        if !self.is_stopping() {
            self.ready.send_replace(true);
        }
    }

    // An owned host handle for calls made while this context is borrowed,
    // such as inside `until_stopping`.
    pub fn host(&self) -> crate::operations::OperationManager {
        crate::operations::OperationManager::new(self.operations.clone())
    }

    pub fn records(&self) -> &crate::records::RecordManager<A> {
        &self.records
    }

    pub fn state(&self) -> &StateManager<A> {
        &self.state
    }

    pub fn streams(&self) -> &StreamManager<A> {
        &self.streams
    }

    pub fn is_stopping(&self) -> bool {
        *self.stopping.borrow()
    }

    pub async fn stopping(&mut self) {
        while !*self.stopping.borrow_and_update() {
            if self.stopping.changed().await.is_err() {
                break;
            }
        }
    }

    pub async fn rest(&mut self, duration: Duration) -> bool {
        let sleep = self.operations.clock().sleep(duration);
        self.until_stopping(sleep).await.is_none()
    }

    pub async fn until_stopping<T>(&mut self, future: impl Future<Output = T>) -> Option<T> {
        if self.is_stopping() {
            return None;
        }
        let mut future = Box::pin(future);
        let mut stop = Box::pin(self.stopping.changed());

        std::future::poll_fn(|context| {
            if let Poll::Ready(Ok(()) | Err(_)) = stop.as_mut().poll(context) {
                return Poll::Ready(None);
            }
            future.as_mut().poll(context).map(Some)
        })
        .await
    }
}

impl<A: Application> harmos_operations::execution::operations::OperationHost for ServiceContext<A> {
    async fn invoke<O: crate::operations::RequestShape, E: harmos_operations::execution::operations::EndpointSet>(
        &self, request: O::Request, retry: Option<crate::RetryPolicy>, endpoints: E,
    ) -> Result<Self::Instance<O>> {
        harmos_operations::execution::operations::OperationHost::invoke::<O, E>(&self.operations, request, retry, endpoints).await
    }

    fn publish_on<S: harmos_operations::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        harmos_operations::streams::publish_on::<_, S>(self, row)
    }

    type Error = Error;
    type Instance<O: RequestShape> = Running<O>;

    fn context(&self) -> &CallContext {
        harmos_operations::execution::operations::OperationHost::context(&self.operations)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let operations = harmos_operations::execution::operations::OperationHost::with_value(&self.operations, value)?;
        let mut child = self.clone();
        child.operations = operations;
        Ok(child)
    }

    fn submit<O: RequestShape>(&self, request: O::Request) -> impl Future<Output = Result<Running<O>>> + Send {
        harmos_operations::execution::operations::OperationHost::submit::<O>(&self.operations, request)
    }

    fn submit_owned<O: RequestShape>(
        &self, request: O::Request, claims: crate::operations::RequestClaims,
    ) -> impl Future<Output = Result<Running<O>>> + Send {
        harmos_operations::execution::operations::OperationHost::submit_owned::<O>(&self.operations, request, claims)
    }
}

impl<A: Application> Clone for ServiceContext<A> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            records: self.records.clone(),
            operations: self.operations.clone(),
            streams: self.streams.clone(),
            stopping: self.stopping.clone(),
            ready: self.ready.clone(),
        }
    }
}

impl<A, F, Fut, E> Resident<A> for F
where
    A: Application,
    F: FnMut(ServiceContext<A>) -> Fut + Send + 'static,
    Fut: Future<Output = std::result::Result<(), E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    type Error = E;

    fn run(&mut self, context: ServiceContext<A>) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send {
        self(context)
    }

    // Closures must not own asynchronous resources beyond their run future.
    // Resource-owning services implement Resident and retain their cleanup state.
    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(crate) fn validate(definition: &ServiceDefinition) -> Result<()> {
    if definition.id.is_empty() {
        return Err(Error::InvalidInput {
            field: "service name",
            reason: "must not be empty",
        });
    }
    definition.policy.validate().map_err(|source| Error::OperationDeclaration { source: source.into() })
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Model;
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl Application for Model {
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
    }

    #[derive(Default)]
    struct Counts {
        initialized: AtomicUsize,
        stopped: AtomicUsize,
        cleaned: AtomicUsize,
        running: tokio::sync::Notify,
    }

    struct Worker {
        counts: Arc<Counts>,
        attempt: usize,
    }

    impl harmos_operations::execution::Service for Worker {
        type Error = crate::Error;

        async fn initialize(context: harmos_operations::execution::ExecutionContext) -> Result<Self> {
            let counts = context.resource::<Counts>()?;
            let attempt = counts.initialized.fetch_add(1, Ordering::SeqCst);
            Ok(Self { counts, attempt })
        }

        async fn run(&mut self, context: harmos_operations::execution::ExecutionContext) -> Result<()> {
            if self.attempt == 0 {
                return Err(Error::InvalidInput {
                    field: "fixture",
                    reason: "first attempt fails",
                });
            }
            let context = context.resource::<ServiceContext<Model>>()?;
            self.counts.running.notify_one();
            let mut context = (*context).clone();
            context.stopping().await;
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            self.counts.stopped.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn cleanup(&mut self) -> Result<()> {
            self.counts.cleaned.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn authored_service_reconstructs_after_cleanup_and_stops_on_withdrawal() {
        use crate::operations::RegistrationExt;

        let counts = Arc::new(Counts::default());
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource::<Counts>(counts.clone());
        })
        .await
        .unwrap();
        let policy = ServicePolicy {
            stop_timeout: Duration::from_secs(1),
            restart: RestartPolicy::OnFailure {
                credit: 1,
                sustained_health: Duration::from_secs(1),
                backoff: Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: Jitter::None,
                },
            },
        };
        let batch = harmos_operations::execution::__private::Catalog::new().service::<Worker>("worker", policy);
        runtime.register_batch(batch.lane(crate::operations::Lane::default())).await.unwrap();
        executor::timeout(Duration::from_secs(2), counts.running.notified()).await.unwrap();
        assert_eq!(counts.initialized.load(Ordering::SeqCst), 2);
        assert_eq!(counts.cleaned.load(Ordering::SeqCst), 1);

        runtime.unregister(&crate::QualifiedId::parse("host.worker").unwrap()).await.unwrap();
        assert_eq!(counts.stopped.load(Ordering::SeqCst), 2);
        assert_eq!(counts.cleaned.load(Ordering::SeqCst), 2);
        runtime.shutdown().await.unwrap();
        assert_eq!(counts.initialized.load(Ordering::SeqCst), 2);
    }
}
