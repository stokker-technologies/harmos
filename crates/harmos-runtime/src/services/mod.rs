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

#[cfg(feature = "plugin-sidecar")]
mod sidecar;

/* === Re-Exports === */

pub(crate) use runner::{Once, Typed};

#[cfg(feature = "plugin-sidecar")]
pub(crate) use sidecar::Endpoint as NativeEndpoint;
#[cfg(feature = "plugin-sidecar")]
pub(crate) use sidecar::RuntimeDriver;

#[cfg(feature = "plugin-sidecar")]
pub use sidecar::{DemandLease, DemandSource, Managed, ManagedError, Notice, SidecarError, Supervised};

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
use crate::operations::{
    Backoff, CallContext, ContextError, ContextValue, Jitter, Manager, OperationDeclaration, Response, RestartPolicy, Running,
};
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

pub(crate) type ServiceRunner =
    fn(ServiceDefinition, harmos_sdk::operations::ServiceFactory, ServiceOwner) -> Arc<dyn crate::operations::resident::Runner>;

#[derive(Clone)]
pub(crate) enum ServiceOwner {
    Host,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    Plugin(crate::plugin::PluginScope),
}

pub(crate) fn authored_runner<A: Application>(
    definition: ServiceDefinition, factory: harmos_sdk::operations::ServiceFactory, owner: ServiceOwner,
) -> Arc<dyn crate::operations::resident::Runner> {
    Arc::new(Typed::<A, harmos_sdk::ExecutionContext, _>::new(definition, move |_| AuthoredService {
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
    pub(crate) storage: Arc<std::sync::OnceLock<crate::storage::StorageManager<crate::storage::ErasedCatalog>>>,
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
    factory: harmos_sdk::operations::ServiceFactory,
    instance: Option<Box<dyn harmos_sdk::operations::ServiceInstance>>,
}

pub struct ServiceContext<A: Application> {
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::records::RecordManager<A>,
    pub(crate) operations: Manager,
    pub(crate) streams: StreamManager<A>,
    pub(crate) stopping: watch::Receiver<bool>,
    pub(crate) ready: watch::Sender<bool>,

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) provider: Arc<crate::supervision::instance::Instance>,

    #[cfg(feature = "plugin-sidecar")]
    deactivation: Arc<std::sync::atomic::AtomicBool>,
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
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) fn scoped(operations: Manager, scope: crate::plugin::PluginScope) -> Self {
        Self {
            operations,
            owner: ServiceOwner::Plugin(scope),
        }
    }
    fn owner(&self) -> &str {
        match &self.owner {
            ServiceOwner::Host => "host",
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            ServiceOwner::Plugin(scope) => scope.owner(),
        }
    }

    pub async fn register<S: harmos_sdk::Service>(&self, id: &str, policy: crate::ServicePolicy) -> Result<()> {
        let batch = harmos_sdk::__private::RegistrationBatch::new().service::<S>(id, policy);
        self.register_batch(batch).await
    }

    pub(crate) async fn register_batch(&self, batch: harmos_sdk::__private::RegistrationBatch) -> Result<()> {
        let (staged, ids) = self.stage(batch)?;
        staged.commit()?;
        self.activate(&ids).await
    }

    pub(crate) fn stage(
        &self, batch: harmos_sdk::__private::RegistrationBatch,
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
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
    type Error = harmos_sdk::ServingError;

    async fn run(&mut self, context: ServiceContext<A>) -> std::result::Result<(), Self::Error> {
        let mut stopping = context.stopping.clone();
        let context = Arc::new(context);
        let mut dependencies = harmos_sdk::__private::ResourceSet::new();
        match &self.owner {
            ServiceOwner::Host => dependencies.insert(context.clone()).map_err(|_| harmos_sdk::ServingError::Catalog)?,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            ServiceOwner::Plugin(scope) => dependencies.insert(Arc::new(scope.clone())).map_err(|_| harmos_sdk::ServingError::Catalog)?,
        }
        let call = harmos_sdk::ExecutionContext::new(dependencies, harmos_sdk::operations::Host::context(&context.operations).clone())
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
    pub(crate) fn context(
        &self, operations: Manager, stopping: watch::Receiver<bool>, ready: watch::Sender<bool>,
        #[cfg(feature = "plugin-sidecar")] provider: Arc<crate::supervision::instance::Instance>,
    ) -> ServiceContext<A> {
        ServiceContext {
            state: self.state.clone(),
            records: self.records.clone(),
            operations,
            streams: self.streams.clone(),
            stopping,
            ready,
            #[cfg(feature = "plugin-sidecar")]
            provider,
            #[cfg(feature = "plugin-sidecar")]
            deactivation: Arc::default(),
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
    pub fn resource<T: Send + Sync + 'static>(&self) -> std::result::Result<Arc<T>, harmos_sdk::ResourceError> {
        self.operations.resources().resource::<T>()
    }

    pub fn ready(&self) {
        if !self.is_stopping() {
            self.ready.send_replace(true);
        }
    }

    // An owned host handle for calls made while this context is borrowed,
    // such as inside `until_stopping`.
    pub fn host(&self) -> crate::operations::OperationManager<A::Lanes> {
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
        self.until_stopping(executor::sleep(duration)).await.is_none()
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

impl<A: Application> harmos_sdk::operations::Host for ServiceContext<A> {
    fn publish_on<S: harmos_domain::StreamSpec>(
        &self, row: &S::Row,
    ) -> impl std::future::Future<Output = std::result::Result<(), Self::Error>> + Send
    where
        S::Row: harmos_domain::Message,
    {
        harmos_operations::streams::publish_on::<_, S>(self, row)
    }

    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;

    fn context(&self) -> &CallContext {
        harmos_sdk::operations::Host::context(&self.operations)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let operations = harmos_sdk::operations::Host::with_value(&self.operations, value)?;
        let mut child = self.clone();
        child.operations = operations;
        Ok(child)
    }

    fn submit<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Running<O>>> + Send {
        harmos_sdk::operations::Host::submit::<O>(&self.operations, request)
    }

    fn submit_owned<O: OperationDeclaration>(
        &self, request: O::Request, claims: crate::operations::RequestClaims,
    ) -> impl Future<Output = Result<Running<O>>> + Send {
        harmos_sdk::operations::Host::submit_owned::<O>(&self.operations, request, claims)
    }

    fn call<O: OperationDeclaration>(&self, request: O::Request) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        harmos_sdk::operations::Host::call::<O>(&self.operations, request)
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
            #[cfg(feature = "plugin-sidecar")]
            provider: self.provider.clone(),
            #[cfg(feature = "plugin-sidecar")]
            deactivation: self.deactivation.clone(),
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

    impl harmos_sdk::Service for Worker {
        type Error = crate::Error;

        async fn initialize(context: harmos_sdk::ExecutionContext) -> Result<Self> {
            let counts = context.resource::<Counts>()?;
            let attempt = counts.initialized.fetch_add(1, Ordering::SeqCst);
            Ok(Self { counts, attempt })
        }

        async fn run(&mut self, context: harmos_sdk::ExecutionContext) -> Result<()> {
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
        let batch = harmos_sdk::__private::RegistrationBatch::new().service::<Worker>("worker", policy);
        runtime.register_batch(batch.lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
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
