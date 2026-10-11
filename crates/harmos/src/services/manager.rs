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
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::{
    Fault, Resident, Service, ServiceExit, ServiceLifecycle, ServiceOutcome, ServicePhase,
    ServicePolicy, ServiceStatus,
};
use crate::observers::ObserverManager;
use crate::supervision::{self, Task};
use crate::{Application, Error, Evidence, Result, StateManager, StreamManager};

/* === Constants === */
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */
pub(super) type Outcome = std::result::Result<(), Arc<Error>>;
type Resources = Arc<dyn Any + Send + Sync>;

/* === Structs === */
struct Physical<R> {
    resident: R,
    stopped: bool,
    lifecycle: Option<Arc<supervision::instance::Instance>>,
    publish: Option<Box<dyn Fn() + Send>>,
}

pub struct ServiceManager<A: Application> {
    inner: Arc<Services<A>>,
}

#[derive(Clone, Debug)]
pub struct ServiceDefinition {
    pub id: String,
    pub policy: ServicePolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceScopeId(u64);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceKey {
    pub definition: String,
    pub scope: ServiceScopeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceInstanceId(u64);

#[derive(Clone, Debug)]
pub struct ServiceSnapshot {
    pub id: ServiceInstanceId,
    pub leases: usize,
    pub status: ServiceStatus,
    pub lifecycle: ServiceLifecycle,
    pub failure: Option<Arc<Error>>,
}

pub struct ServiceInstance<A: Application> {
    task: Running<A>,
}

pub struct ServiceScope<R> {
    scope: Arc<Scope>,
    marker: PhantomData<fn() -> R>,
}

#[must_use = "a service lease holds activation demand"]
pub struct ServiceLease<A: Application> {
    owner: Weak<Services<A>>,
    key: ServiceKey,
    instance_id: ServiceInstanceId,
    demand_id: u64,
    task: Running<A>,
    released: bool,
    joins_release: bool,
}

pub(crate) struct Definition<A: Application> {
    pub(crate) name: String,
    pub(super) policy: ServicePolicy,
    pub(super) resident: Box<dyn ErasedResident<A>>,
}

pub(super) struct Failure {
    pub(super) message: String,
    pub(super) source: Arc<dyn std::error::Error + Send + Sync>,
}

pub(crate) struct Handles<A: Application> {
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::RecordManager<A>,
    pub(crate) operations: crate::OperationManager,
    pub(crate) streams: StreamManager<A>,
    pub(crate) evidence: Evidence<A>,
    pub(crate) observers: ObserverManager,
}

pub(super) struct Services<A: Application> {
    identity: Arc<()>,
    pub(super) state: StateManager<A>,
    pub(super) records: crate::RecordManager<A>,
    pub(super) operations: crate::OperationManager,
    pub(super) streams: StreamManager<A>,
    pub(super) evidence: Evidence<A>,
    pub(super) observers: ObserverManager,
    root: Arc<Scope>,
    registry: Mutex<Registry<A>>,
    statuses: watch::Sender<BTreeMap<ServiceKey, ServiceSnapshot>>,
    assembly: Mutex<Vec<ServiceLease<A>>>,
}

pub(super) struct Activation<A: Application> {
    pub(super) services: Arc<Services<A>>,
    key: ServiceKey,
    instance_id: ServiceInstanceId,
}

struct Scope {
    owner: Weak<()>,
    id: ServiceScopeId,
    resources: Resources,
    closed: AtomicBool,
}

struct Registry<A: Application> {
    closed: bool,
    next: u64,
    reserved: BTreeSet<String>,
    definitions: BTreeMap<String, Arc<Registration<A>>>,
    scopes: BTreeMap<ServiceScopeId, Arc<Scope>>,
    instances: BTreeMap<ServiceKey, Instance<A>>,
}

struct Registration<A: Application> {
    definition: ServiceDefinition,
    factory: Arc<dyn Factory<A>>,
    scope: Option<ServiceScopeId>,
    internal: bool,
    closing: AtomicBool,
}

struct Instance<A: Application> {
    view: ServiceSnapshot,
    leases: BTreeSet<u64>,
    task: Running<A>,
    stopping: bool,
}

struct TypedFactory<R, F>(F, PhantomData<fn() -> R>);

struct Once<A: Application>(Mutex<Option<Box<dyn ErasedResident<A>>>>);

pub(super) struct Cleanup<A: Application> {
    pub(super) lifecycle: Arc<supervision::instance::Instance>,
    resident: OnceLock<supervision::cleanup::Cleanup<Box<dyn ErasedResident<A>>>>,
    pub(super) finalized: AtomicBool,
    pub(super) logical: Mutex<Option<Outcome>>,
}

struct Running<A: Application> {
    task: Arc<Task<Outcome>>,
    cleanup: Arc<Cleanup<A>>,
    owner: Weak<Services<A>>,
    key: ServiceKey,
    instance_id: ServiceInstanceId,
}

/* === Enums === */
enum Acquisition<A: Application> {
    Lease(ServiceLease<A>),
    Wait(ServiceInstanceId, Running<A>),
}

/* === Traits === */
pub(super) trait ErasedResident<A: Application>: Send {
    fn bind_lifecycle(
        &mut self,
        lifecycle: Arc<supervision::instance::Instance>,
        publish: Box<dyn Fn() + Send>,
    );
    fn attempt(
        &mut self,
        context: Service<A>,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), Fault<Failure>>> + Send + '_>>;

    fn finalize(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), Failure>> + Send + '_>>;
}

trait Factory<A: Application>: Send + Sync {
    fn resources(&self) -> TypeId;
    fn create(
        &self,
        resources: Resources,
        admitted: bool,
    ) -> Result<Option<Box<dyn ErasedResident<A>>>>;
}

/* === Implementations === */
impl<A: Application> supervision::cleanup::Resource for Box<dyn ErasedResident<A>> {
    fn finalize(&mut self) -> supervision::cleanup::Finalize<'_> {
        Box::pin(async move {
            super::supervisor::finalize(self.as_mut())
                .await
                .map_err(Arc::new)
        })
    }
}

impl<A, R> ErasedResident<A> for Physical<R>
where
    A: Application,
    R: Resident<A>,
{
    fn bind_lifecycle(
        &mut self,
        lifecycle: Arc<supervision::instance::Instance>,
        publish: Box<dyn Fn() + Send>,
    ) {
        self.lifecycle = Some(lifecycle);
        self.publish = Some(publish);
    }

    fn attempt(
        &mut self,
        context: Service<A>,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), Fault<Failure>>> + Send + '_>> {
        Box::pin(async move {
            self.stopped = false;
            Resident::attempt(&mut self.resident, context)
                .await
                .map_err(|fault| match fault {
                    Fault::Environmental(error) => Fault::Environmental(Failure {
                        message: error.to_string(),
                        source: Arc::new(Fault::Environmental(error)),
                    }),
                    Fault::Application(error) => Fault::Application(Failure {
                        message: error.to_string(),
                        source: Arc::new(Fault::Application(error)),
                    }),
                })
        })
    }
    fn finalize(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), Failure>> + Send + '_>> {
        Box::pin(async move {
            let lifecycle = self
                .lifecycle
                .as_ref()
                .expect("resident bound before execution");
            if !self.stopped {
                lifecycle.transition(ServicePhase::Stopping);
                if let Some(publish) = &self.publish {
                    publish();
                }
                Resident::stop(&mut self.resident)
                    .await
                    .map_err(|error| Failure {
                        message: error.to_string(),
                        source: Arc::new(Fault::Application(error)),
                    })?;
                self.stopped = true;
            }
            lifecycle.transition(ServicePhase::Finalizing);
            if let Some(publish) = &self.publish {
                publish();
            }
            Resident::finalize(&mut self.resident)
                .await
                .map_err(|error| Failure {
                    message: error.to_string(),
                    source: Arc::new(Fault::Application(error)),
                })
        })
    }
}

impl<A, R, F, T> Factory<A> for TypedFactory<R, F>
where
    A: Application,
    R: Send + Sync + 'static,
    F: Fn(Arc<R>) -> T + Send + Sync + 'static,
    T: Resident<A>,
{
    fn resources(&self) -> TypeId {
        TypeId::of::<R>()
    }
    fn create(
        &self,
        resources: Resources,
        admitted: bool,
    ) -> Result<Option<Box<dyn ErasedResident<A>>>> {
        if !admitted {
            return Ok(None);
        }
        let resources = resources.downcast::<R>().map_err(|_| Error::InvalidInput {
            field: "service scope",
            reason: "resource type differs from the registered factory",
        })?;
        Ok(Some(Box::new(Physical {
            resident: (self.0)(resources),
            stopped: false,
            lifecycle: None,
            publish: None,
        })))
    }
}

impl<A: Application> Factory<A> for Once<A> {
    fn resources(&self) -> TypeId {
        TypeId::of::<()>()
    }
    fn create(&self, _: Resources, _: bool) -> Result<Option<Box<dyn ErasedResident<A>>>> {
        let resident = self.0.lock().unwrap().take().ok_or(Error::NotReady {
            domain: "retired assembly service",
        })?;
        Ok(Some(resident))
    }
}

impl<A: Application> Definition<A> {
    pub(crate) fn new<R: Resident<A>>(name: String, policy: ServicePolicy, resident: R) -> Self {
        Self {
            name,
            policy,
            resident: Box::new(Physical {
                resident,
                stopped: false,
                lifecycle: None,
                publish: None,
            }),
        }
    }
}

impl ServiceKey {
    pub fn root(id: impl Into<String>) -> Self {
        Self {
            definition: id.into(),
            scope: ServiceScopeId(0),
        }
    }
}

impl<R> ServiceScope<R> {
    pub fn id(&self) -> ServiceScopeId {
        self.scope.id
    }
}

impl<R> Clone for ServiceScope<R> {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope.clone(),
            marker: PhantomData,
        }
    }
}

impl<A: Application> Clone for ServiceManager<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<A: Application> ServiceManager<A> {
    pub(crate) fn new(
        handles: Handles<A>,
        declared: &[Definition<A>],
        identity: Arc<()>,
    ) -> Result<Self> {
        let mut reserved = BTreeSet::new();
        for item in declared {
            validate_name(&item.name)?;
            if !reserved.insert(item.name.clone()) {
                return Err(Error::AlreadyRegistered {
                    domain: "service",
                    name: item.name.clone(),
                });
            }
        }
        let root = Arc::new(Scope {
            owner: Arc::downgrade(&identity),
            id: ServiceScopeId(0),
            resources: Arc::new(()),
            closed: AtomicBool::new(false),
        });
        Ok(Self {
            inner: Arc::new(Services {
                identity,
                records: handles.records,
                operations: handles.operations,
                state: handles.state,
                streams: handles.streams,
                evidence: handles.evidence,
                observers: handles.observers,
                root: root.clone(),
                registry: Mutex::new(Registry {
                    closed: false,
                    next: 0,
                    reserved,
                    definitions: BTreeMap::new(),
                    scopes: BTreeMap::from([(root.id, root)]),
                    instances: BTreeMap::new(),
                }),
                statuses: watch::channel(BTreeMap::new()).0,
                assembly: Mutex::new(Vec::new()),
            }),
        })
    }
    pub(crate) fn identity(&self) -> Arc<()> {
        self.inner.identity.clone()
    }
    pub fn root_scope(&self) -> ServiceScope<()> {
        ServiceScope {
            scope: self.inner.root.clone(),
            marker: PhantomData,
        }
    }

    pub fn register<R, F, T>(&self, definition: ServiceDefinition, factory: F) -> Result<()>
    where
        R: Send + Sync + 'static,
        F: Fn(Arc<R>) -> T + Send + Sync + 'static,
        T: Resident<A>,
    {
        validate_name(&definition.id)?;
        self.register_factory(
            definition,
            Arc::new(TypedFactory(factory, PhantomData)),
            None,
            false,
            false,
        )
    }
    fn register_factory(
        &self,
        definition: ServiceDefinition,
        factory: Arc<dyn Factory<A>>,
        scope: Option<ServiceScopeId>,
        internal: bool,
        boot: bool,
    ) -> Result<()> {
        let mut registry = self.inner.registry.lock().unwrap();
        registry.admit()?;
        let id = definition.id.clone();
        if registry.definitions.contains_key(&id) || (!boot && registry.reserved.contains(&id)) {
            return Err(Error::AlreadyRegistered {
                domain: "service",
                name: id,
            });
        }
        registry.reserved.remove(&id);
        registry.definitions.insert(
            id,
            Arc::new(Registration {
                definition,
                factory,
                scope,
                internal,
                closing: AtomicBool::new(false),
            }),
        );
        Ok(())
    }
    pub fn definition(&self, id: &str) -> Option<ServiceDefinition> {
        self.inner
            .registry
            .lock()
            .unwrap()
            .definitions
            .get(id)
            .map(|item| item.definition.clone())
    }
    pub fn definitions(&self) -> Vec<ServiceDefinition> {
        self.inner
            .registry
            .lock()
            .unwrap()
            .definitions
            .values()
            .map(|item| item.definition.clone())
            .collect()
    }
    pub fn scope<R: Send + Sync + 'static>(&self, resources: Arc<R>) -> Result<ServiceScope<R>> {
        let mut registry = self.inner.registry.lock().unwrap();
        registry.admit()?;
        let id = ServiceScopeId(
            NEXT_SCOPE
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .map_err(|_| Error::InvalidInput {
                    field: "service scope",
                    reason: "identity space exhausted",
                })?,
        );
        let scope = Arc::new(Scope {
            owner: Arc::downgrade(&self.inner.identity),
            id,
            resources,
            closed: AtomicBool::new(false),
        });
        registry.scopes.insert(id, scope.clone());
        Ok(ServiceScope {
            scope,
            marker: PhantomData,
        })
    }
    fn check_scope<R>(&self, scope: &ServiceScope<R>) -> Result<()> {
        if !Weak::ptr_eq(&scope.scope.owner, &Arc::downgrade(&self.inner.identity)) {
            return Err(Error::InvalidInput {
                field: "service scope",
                reason: "scope belongs to another manager",
            });
        }
        Ok(())
    }
    pub async fn activate<R: Send + Sync + 'static>(
        &self,
        id: &str,
        scope: &ServiceScope<R>,
    ) -> Result<ServiceLease<A>> {
        let mut joined = None;
        loop {
            match self.acquire(id, scope, joined, false)? {
                Acquisition::Lease(lease) => return Ok(lease),
                Acquisition::Wait(instance_id, task) => {
                    let result = settle(vec![task.clone()]).await;
                    if !task.cleanup.lifecycle.is_settled() {
                        result?;
                        return Err(Error::NotReady {
                            domain: "service cleanup",
                        });
                    }
                    joined = Some(instance_id);
                }
            }
        }
    }
    fn acquire<R: Send + Sync + 'static>(
        &self,
        id: &str,
        scope: &ServiceScope<R>,
        joined: Option<ServiceInstanceId>,
        internal: bool,
    ) -> Result<Acquisition<A>> {
        self.check_scope(scope)?;
        let mut registry = self.inner.registry.lock().unwrap();
        registry.admit()?;
        if scope.scope.closed.load(Ordering::Acquire) {
            return Err(Error::Closing {
                domain: "service scope",
            });
        }
        let registration =
            registry
                .definitions
                .get(id)
                .cloned()
                .ok_or_else(|| Error::NotRegistered {
                    domain: "service",
                    name: id.to_owned(),
                })?;
        if registration.internal && !internal {
            return Err(Error::InvalidInput {
                field: "service definition",
                reason: "infrastructure demand belongs to its runtime owner",
            });
        }
        if registration.closing.load(Ordering::Acquire) {
            return Err(Error::Closing {
                domain: "service definition",
            });
        }
        if registration.scope.is_some_and(|owner| owner != scope.id())
            || registration.factory.resources() != TypeId::of::<R>()
        {
            return Err(Error::InvalidInput {
                field: "service scope",
                reason: "definition belongs to another resource scope or type",
            });
        }
        let key = ServiceKey {
            definition: id.to_owned(),
            scope: scope.id(),
        };
        let lease_id = registry.next()?;
        if let Some(instance) = registry.instances.get_mut(&key) {
            if !instance.stopping && instance.task.cleanup.lifecycle.status().outcome().is_none() {
                instance.leases.insert(lease_id);
                instance.view.leases = instance.leases.len();
                let lease = self.lease(&key, instance, lease_id);
                self.inner.publish(&registry);
                return Ok(Acquisition::Lease(lease));
            }
            if joined != Some(instance.view.id) {
                return Ok(Acquisition::Wait(instance.view.id, instance.task.clone()));
            }
            if registration.scope.is_some() {
                return Err(Error::NotReady {
                    domain: "retired assembly service",
                });
            }
        }
        let instance_id = ServiceInstanceId(registry.next()?);
        let services = self.inner.clone();
        let activation_key = key.clone();
        let resources = scope.scope.resources.clone();
        let stopping = watch::channel(false).0;
        let cleanup = Arc::new(Cleanup {
            lifecycle: Arc::new(supervision::instance::Instance::queued(stopping.clone())),
            resident: OnceLock::new(),
            finalized: AtomicBool::new(true),
            logical: Mutex::new(None),
        });
        let retained = cleanup.clone();
        let task = Arc::new(Task::with_cancellation(
            stopping,
            move |stopping| async move {
                let activation = Activation {
                    services: services.clone(),
                    key: activation_key.clone(),
                    instance_id,
                };
                let cleanup = retained.clone();
                let outcome = supervision::joined(async move {
                    let admitted = cleanup.lifecycle.initialize_attempt(0);
                    let Some(mut resident) = registration.factory.create(resources, admitted)?
                    else {
                        return Ok(());
                    };
                    let owner = Arc::downgrade(&activation.services);
                    resident.bind_lifecycle(
                        cleanup.lifecycle.clone(),
                        Box::new(move || {
                            if let Some(services) = owner.upgrade() {
                                let registry = services.registry.lock().unwrap();
                                services.publish(&registry);
                            }
                        }),
                    );
                    assert!(
                        cleanup
                            .resident
                            .set(supervision::cleanup::Cleanup::new(resident))
                            .is_ok(),
                        "a service activation creates one physical resident"
                    );
                    let mut original = cleanup
                        .resident
                        .get()
                        .expect("created resident is retained")
                        .take()
                        .expect("the service supervisor owns its resident");
                    cleanup.finalized.store(false, Ordering::Release);
                    super::supervisor::run(
                        registration.definition.id.clone(),
                        registration.definition.policy,
                        original.resource().as_mut(),
                        &cleanup,
                        activation,
                        stopping,
                    )
                    .await
                })
                .await
                .map_err(|source| Error::TaskFailed {
                    domain: "service supervisor",
                    source: Arc::new(source),
                })
                .and_then(|answer| answer)
                .map_err(Arc::new);
                retained.record(outcome.clone());
                let outcome = match retained.resident.get() {
                    Some(resident) if retained.finalized.load(Ordering::Acquire) => {
                        match resident.release().await {
                            Ok(()) => {
                                retained.lifecycle.finished();
                                outcome
                            }
                            Err(error) => Err(error),
                        }
                    }
                    None => {
                        retained.lifecycle.finished();
                        outcome
                    }
                    Some(_) => outcome,
                };
                services.complete(&activation_key, instance_id, &outcome);
                outcome
            },
        ));
        let task = Running {
            task,
            cleanup,
            owner: Arc::downgrade(&self.inner),
            key: key.clone(),
            instance_id,
        };
        let instance = Instance {
            view: ServiceSnapshot {
                id: instance_id,
                leases: 1,
                status: ServiceStatus::Starting,
                lifecycle: task.cleanup.lifecycle.status(),
                failure: None,
            },
            leases: BTreeSet::from([lease_id]),
            task,
            stopping: false,
        };
        let lease = self.lease(&key, &instance, lease_id);
        let retired = registry.instances.insert(key, instance);
        self.inner.publish(&registry);
        drop(registry);
        drop(retired);
        Ok(Acquisition::Lease(lease))
    }
    fn lease(&self, key: &ServiceKey, instance: &Instance<A>, id: u64) -> ServiceLease<A> {
        ServiceLease {
            owner: Arc::downgrade(&self.inner),
            key: key.clone(),
            instance_id: instance.view.id,
            demand_id: id,
            task: instance.task.clone(),
            released: false,
            joins_release: false,
        }
    }
    pub(crate) fn start(&self, declared: Vec<Definition<A>>) -> Result<()> {
        for definition in declared {
            let lease = self.start_once(definition, false, true)?;
            self.inner.assembly.lock().unwrap().push(lease);
        }
        Ok(())
    }
    fn start_once(
        &self,
        definition: Definition<A>,
        internal: bool,
        boot: bool,
    ) -> Result<ServiceLease<A>> {
        let id = definition.name.clone();
        validate_name(&id)?;
        self.register_factory(
            ServiceDefinition {
                id: id.clone(),
                policy: definition.policy,
            },
            Arc::new(Once(Mutex::new(Some(definition.resident)))),
            Some(ServiceScopeId(0)),
            internal,
            boot,
        )?;
        match self.acquire(&id, &self.root_scope(), None, internal)? {
            Acquisition::Lease(lease) => Ok(lease),
            Acquisition::Wait(..) => {
                unreachable!("a newly registered assembly definition has no instance")
            }
        }
    }
    pub(crate) fn activate_internal<R: Resident<A>>(
        &self,
        name: String,
        policy: ServicePolicy,
        resident: R,
    ) -> Result<ServiceLease<A>> {
        self.start_once(Definition::new(name, policy, resident), true, false)
    }
    pub fn status(&self, key: &ServiceKey) -> Option<ServiceSnapshot> {
        self.inner.statuses.borrow().get(key).cloned()
    }
    pub fn statuses(&self) -> BTreeMap<ServiceKey, ServiceSnapshot> {
        self.inner.statuses.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<BTreeMap<ServiceKey, ServiceSnapshot>> {
        self.inner.statuses.subscribe()
    }

    pub async fn stop_scope<R>(&self, scope: &ServiceScope<R>) -> Result<()> {
        self.check_scope(scope)?;
        let tasks = {
            let mut registry = self.inner.registry.lock().unwrap();
            scope.scope.closed.store(true, Ordering::Release);
            for registration in registry
                .definitions
                .values()
                .filter(|item| item.scope == Some(scope.id()))
            {
                registration.closing.store(true, Ordering::Release);
            }
            let tasks = registry.stop(|key, _| key.scope == scope.id());
            self.inner.publish(&registry);
            tasks
        };
        let retained = tasks.clone();
        let result = settle(tasks).await;
        if retained
            .iter()
            .any(|task| !task.cleanup.lifecycle.is_settled())
        {
            return result;
        }
        let (resources, definitions, instances) = {
            let mut registry = self.inner.registry.lock().unwrap();
            let keys: Vec<_> = registry
                .instances
                .keys()
                .filter(|key| key.scope == scope.id())
                .cloned()
                .collect();
            let instances: Vec<_> = keys
                .into_iter()
                .filter_map(|key| registry.instances.remove(&key))
                .collect();
            let resources = registry.scopes.remove(&scope.id());
            let ids: Vec<_> = registry
                .definitions
                .iter()
                .filter(|(_, value)| value.scope == Some(scope.id()))
                .map(|(id, _)| id.clone())
                .collect();
            let definitions: Vec<_> = ids
                .into_iter()
                .filter_map(|id| registry.definitions.remove(&id))
                .collect();
            self.inner.publish(&registry);
            (resources, definitions, instances)
        };
        drop((resources, definitions, instances));
        result
    }
    pub async fn unregister(&self, id: &str) -> Result<()> {
        let (registration, tasks) = {
            let mut registry = self.inner.registry.lock().unwrap();
            let registration =
                registry
                    .definitions
                    .get(id)
                    .cloned()
                    .ok_or_else(|| Error::NotRegistered {
                        domain: "service",
                        name: id.to_owned(),
                    })?;
            registration.closing.store(true, Ordering::Release);
            let tasks = registry.stop(|key, _| key.definition == id);
            self.inner.publish(&registry);
            (registration, tasks)
        };
        let retained = tasks.clone();
        let result = settle(tasks).await;
        if retained
            .iter()
            .any(|task| !task.cleanup.lifecycle.is_settled())
        {
            return result;
        }
        let mut registry = self.inner.registry.lock().unwrap();
        let mut retired = Vec::new();
        if registry
            .definitions
            .get(id)
            .is_some_and(|current| Arc::ptr_eq(current, &registration))
        {
            registry.definitions.remove(id);
            let keys: Vec<_> = registry
                .instances
                .keys()
                .filter(|key| key.definition == id)
                .cloned()
                .collect();
            retired.extend(
                keys.into_iter()
                    .filter_map(|key| registry.instances.remove(&key)),
            );
            self.inner.publish(&registry);
        }
        drop(registry);
        drop((registration, retired));
        result
    }
    pub(crate) fn cleanup_pending(&self, include_infrastructure: bool) -> bool {
        let registry = self.inner.registry.lock().unwrap();
        registry.instances.iter().any(|(key, instance)| {
            let internal = registry
                .definitions
                .get(&key.definition)
                .is_some_and(|item| item.internal);
            (include_infrastructure || !internal) && !instance.task.cleanup.lifecycle.is_settled()
        })
    }

    pub(crate) fn close_admission(&self) {
        let mut registry = self.inner.registry.lock().unwrap();
        registry.closed = true;
        registry.stop(|_, internal| !internal);
        self.inner.publish(&registry);
    }
    pub(crate) async fn shutdown_application(&self) -> Result<()> {
        self.shutdown_part(false).await
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_part(true).await
    }
    async fn shutdown_part(&self, all: bool) -> Result<()> {
        let tasks = {
            let mut registry = self.inner.registry.lock().unwrap();
            registry.closed = true;
            let tasks = registry.stop(|_, internal| all || !internal);
            self.inner.publish(&registry);
            tasks
        };
        let retained = tasks.clone();
        let result = settle(tasks).await;
        if retained
            .iter()
            .any(|task| !task.cleanup.lifecycle.is_settled())
        {
            return result;
        }
        if all {
            let (definitions, scopes) = {
                let mut registry = self.inner.registry.lock().unwrap();
                for scope in registry.scopes.values() {
                    scope.closed.store(true, Ordering::Release);
                }
                (
                    std::mem::take(&mut registry.definitions),
                    std::mem::take(&mut registry.scopes),
                )
            };
            let assembly = std::mem::take(&mut *self.inner.assembly.lock().unwrap());
            drop((definitions, scopes, assembly));
        }
        result
    }
}

impl<A: Application> Cleanup<A> {
    pub(super) fn record(&self, outcome: Outcome) {
        let mut logical = self.logical.lock().unwrap();
        if logical.is_some() {
            return;
        }
        let state = match &outcome {
            Ok(()) if self.lifecycle.status().cancellation_requested() => ServiceOutcome::Cancelled,
            Ok(()) => ServiceOutcome::Succeeded,
            Err(error) => match error.as_ref() {
                Error::TaskFailed { source, .. }
                    if matches!(
                        source.downcast_ref::<ServiceExit>(),
                        Some(ServiceExit::StopTimedOut)
                    ) =>
                {
                    ServiceOutcome::TimedOut
                }
                _ => ServiceOutcome::Failed,
            },
        };
        *logical = Some(outcome);
        self.lifecycle.complete(state);
    }
}

impl<A: Application> Services<A> {
    fn publish(&self, registry: &Registry<A>) {
        self.statuses.send_replace(
            registry
                .instances
                .iter()
                .map(|(key, value)| {
                    let mut view = value.view.clone();
                    view.lifecycle = value.task.cleanup.lifecycle.status();
                    (key.clone(), view)
                })
                .collect(),
        );
    }
    fn complete(&self, key: &ServiceKey, instance_id: ServiceInstanceId, outcome: &Outcome) {
        let mut registry = self.registry.lock().unwrap();
        if let Some(instance) = registry
            .instances
            .get_mut(key)
            .filter(|item| item.view.id == instance_id)
        {
            let cleanup = &instance.task.cleanup;
            cleanup.record(outcome.clone());
            if !cleanup.lifecycle.is_settled()
                && let Err(error) = outcome
            {
                cleanup.lifecycle.interrupted(error.clone());
            }
            instance.view.failure = outcome.as_ref().err().cloned();
            if outcome.is_err() && !matches!(instance.view.status, ServiceStatus::Inactive { .. }) {
                instance.view.status = ServiceStatus::Inactive {
                    cause: ServiceExit::Panicked,
                    history: Vec::new(),
                };
            } else if outcome.is_ok() {
                instance.view.status = ServiceStatus::Stopped;
            }
            self.publish(&registry);
        }
    }
    fn release(&self, key: &ServiceKey, instance_id: ServiceInstanceId, id: u64) -> bool {
        let mut registry = self.registry.lock().unwrap();
        let Some(instance) = registry
            .instances
            .get_mut(key)
            .filter(|item| item.view.id == instance_id)
        else {
            return true;
        };
        instance.leases.remove(&id);
        instance.view.leases = instance.leases.len();
        let final_release = instance.leases.is_empty();
        if final_release {
            instance.stop();
        }
        self.publish(&registry);
        final_release
    }
}

impl<A: Application> Activation<A> {
    pub(super) fn cleaning(&self) {
        let registry = self.services.registry.lock().unwrap();
        if let Some(instance) = registry
            .instances
            .get(&self.key)
            .filter(|item| item.view.id == self.instance_id)
        {
            instance
                .task
                .cleanup
                .lifecycle
                .transition(ServicePhase::Stopping);
            self.services.publish(&registry);
        }
    }

    pub(super) fn quiesce(&self) {
        let mut registry = self.services.registry.lock().unwrap();
        if let Some(instance) = registry
            .instances
            .get_mut(&self.key)
            .filter(|item| item.view.id == self.instance_id)
        {
            instance.stopping = true;
            instance.view.status = ServiceStatus::Stopping;
            self.services.publish(&registry);
        }
    }

    pub(super) fn set(&self, _: &str, status: ServiceStatus) {
        let mut registry = self.services.registry.lock().unwrap();
        if let Some(instance) = registry
            .instances
            .get_mut(&self.key)
            .filter(|item| item.view.id == self.instance_id)
            && (!instance.stopping
                || matches!(
                    status,
                    ServiceStatus::Stopped | ServiceStatus::Inactive { .. }
                ))
        {
            let lifecycle = &instance.task.cleanup.lifecycle;
            match &status {
                ServiceStatus::Running { .. } => lifecycle.transition(ServicePhase::Running),
                ServiceStatus::BackingOff => lifecycle.transition(ServicePhase::RetryWaiting),
                _ => {}
            }
            instance.view.status = status;
            self.services.publish(&registry);
        }
    }
}

impl<A: Application> std::ops::Deref for Activation<A> {
    type Target = Services<A>;
    fn deref(&self) -> &Self::Target {
        &self.services
    }
}

impl<A: Application> Registry<A> {
    fn admit(&self) -> Result<()> {
        if self.closed {
            Err(Error::Closing { domain: "service" })
        } else {
            Ok(())
        }
    }
    fn next(&mut self) -> Result<u64> {
        self.next = self.next.checked_add(1).ok_or(Error::InvalidInput {
            field: "service identity",
            reason: "identity space exhausted",
        })?;
        Ok(self.next)
    }
    fn stop(&mut self, predicate: impl Fn(&ServiceKey, bool) -> bool) -> Vec<Running<A>> {
        self.instances
            .iter_mut()
            .filter(|(key, _)| {
                predicate(
                    key,
                    self.definitions
                        .get(&key.definition)
                        .is_some_and(|item| item.internal),
                )
            })
            .map(|(_, item)| {
                item.leases.clear();
                item.view.leases = 0;
                item.stop();
                item.task.clone()
            })
            .collect()
    }
}

impl<A: Application> Instance<A> {
    fn stop(&mut self) {
        self.stopping = true;
        if self.task.cleanup.lifecycle.status().outcome().is_none() {
            self.view.status = ServiceStatus::Stopping;
        }
        self.task.stop();
    }
}

impl<A: Application> Clone for Running<A> {
    fn clone(&self) -> Self {
        Self {
            task: self.task.clone(),
            cleanup: self.cleanup.clone(),
            owner: self.owner.clone(),
            key: self.key.clone(),
            instance_id: self.instance_id,
        }
    }
}

impl<A: Application> Running<A> {
    fn stop(&self) {
        self.cleanup.lifecycle.cancel();
    }

    async fn settle(&self) -> std::result::Result<Arc<Outcome>, Arc<tokio::task::JoinError>> {
        if let Err(failure) = self.cleanup.lifecycle.settle_dependents().await {
            return Ok(Arc::new(Err(failure)));
        }
        let cleanup = self.cleanup.clone();
        let owner = self.owner.clone();
        let key = self.key.clone();
        let instance_id = self.instance_id;
        self.task
            .settle_retry(move || {
                if cleanup.lifecycle.is_settled() {
                    return None;
                }
                Some(async move {
                    cleanup.lifecycle.transition(ServicePhase::Finalizing);
                    let result = {
                        match cleanup.resident.get() {
                            Some(resident) if cleanup.finalized.load(Ordering::Acquire) => {
                                resident.release().await
                            }
                            Some(resident) => resident.settle().await,
                            None => Ok(()),
                        }
                    };
                    let outcome = match result {
                        Ok(()) => {
                            cleanup.finalized.store(true, Ordering::Release);
                            cleanup.lifecycle.finished();
                            cleanup.logical.lock().unwrap().clone().unwrap_or(Ok(()))
                        }
                        Err(error) => Err(error),
                    };
                    if let Some(owner) = owner.upgrade() {
                        owner.complete(&key, instance_id, &outcome);
                    }
                    outcome
                })
            })
            .await
    }
}

impl<A: Application> Clone for ServiceInstance<A> {
    fn clone(&self) -> Self {
        Self {
            task: self.task.clone(),
        }
    }
}

impl<A: Application> ServiceInstance<A> {
    pub fn id(&self) -> ServiceInstanceId {
        self.task.instance_id
    }

    pub fn key(&self) -> &ServiceKey {
        &self.task.key
    }

    pub fn status(&self) -> ServiceLifecycle {
        self.task.cleanup.lifecycle.status()
    }

    pub fn subscribe(&self) -> watch::Receiver<ServiceLifecycle> {
        self.task.cleanup.lifecycle.settlement()
    }

    pub fn cancel(&self) {
        if let Some(owner) = self.task.owner.upgrade() {
            let mut registry = owner.registry.lock().unwrap();
            if let Some(instance) = registry.instances.get_mut(&self.task.key)
                && instance.view.id == self.task.instance_id
            {
                instance.stop();
                owner.publish(&registry);
                return;
            }
        }
        self.task.stop();
    }

    pub async fn join(&self) -> Result<()> {
        settle(vec![self.task.clone()]).await
    }
}

impl<A: Application> ServiceLease<A> {
    pub fn instance(&self) -> ServiceInstance<A> {
        ServiceInstance {
            task: self.task.clone(),
        }
    }

    pub fn lifecycle(&self) -> ServiceLifecycle {
        self.task.cleanup.lifecycle.status()
    }

    pub fn subscribe_lifecycle(&self) -> watch::Receiver<ServiceLifecycle> {
        self.task.cleanup.lifecycle.settlement()
    }

    pub fn key(&self) -> &ServiceKey {
        &self.key
    }
    pub fn id(&self) -> ServiceInstanceId {
        self.instance_id
    }
    pub async fn ready(&self) -> Result<()> {
        let owner = self.owner.upgrade().ok_or(Error::NotReady {
            domain: "service owner",
        })?;
        let mut statuses = owner.statuses.subscribe();
        loop {
            let snapshot = statuses.borrow_and_update().get(&self.key).cloned();
            match snapshot
                .filter(|item| item.id == self.instance_id)
                .map(|item| item.status)
            {
                Some(ServiceStatus::Running { .. }) => return Ok(()),
                Some(ServiceStatus::Starting | ServiceStatus::BackingOff) => {}
                _ => {
                    settle(vec![self.task.clone()]).await?;
                    return Err(Error::NotReady {
                        domain: "service activation",
                    });
                }
            }
            statuses.changed().await.map_err(|_| Error::NotReady {
                domain: "service owner",
            })?;
        }
    }
    pub async fn release(mut self) -> Result<()> {
        self.begin_release().await
    }

    pub(crate) fn retire(&mut self) -> impl Future<Output = Result<()>> + Send + 'static + use<A> {
        let completion = self.begin_release();
        let owner = self.owner.clone();
        let key = self.key.clone();
        let instance_id = self.instance_id;
        let completion_cleanup = self.task.cleanup.clone();
        async move {
            let result = completion.await;
            if !completion_cleanup.lifecycle.is_settled() {
                return result;
            }
            if let Some(owner) = owner.upgrade() {
                let mut registry = owner.registry.lock().unwrap();
                let owns = registry
                    .instances
                    .get(&key)
                    .is_some_and(|item| item.view.id == instance_id)
                    && registry
                        .definitions
                        .get(&key.definition)
                        .is_some_and(|item| item.internal);
                let retired = owns.then(|| {
                    (
                        registry.instances.remove(&key),
                        registry.definitions.remove(&key.definition),
                    )
                });
                owner.publish(&registry);
                drop(registry);
                drop(retired);
            }
            result
        }
    }

    fn begin_release(
        &mut self,
    ) -> impl std::future::Future<Output = Result<()>> + Send + 'static + use<A> {
        self.request();
        let task = self.joins_release.then(|| self.task.clone());
        async move { settle(task.into_iter().collect()).await }
    }
    fn request(&mut self) -> bool {
        if self.released {
            return self.joins_release;
        }
        self.released = true;
        self.joins_release = if let Some(owner) = self.owner.upgrade() {
            owner.release(&self.key, self.instance_id, self.demand_id)
        } else {
            self.task.stop();
            true
        };
        self.joins_release
    }
}

impl<A: Application> Drop for ServiceLease<A> {
    fn drop(&mut self) {
        self.request();
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn settle<A: Application>(tasks: Vec<Running<A>>) -> Result<()> {
    let pending = tasks.into_iter().map(|task| async move {
        match task.settle().await {
            Ok(outcome) => outcome
                .as_ref()
                .clone()
                .map_err(|source| Error::TaskFailed {
                    domain: "service",
                    source,
                }),
            Err(source) => Err(Error::TaskFailed {
                domain: "service supervisor",
                source,
            }),
        }
    });
    supervision::settle_all(pending).await
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidInput {
            field: "service name",
            reason: "must not be empty",
        });
    }
    if name == crate::evidence::SOURCE {
        return Err(Error::InvalidInput {
            field: "service name",
            reason: "artifact is reserved for evidence facts",
        });
    }
    Ok(())
}

/* === Tests === */
