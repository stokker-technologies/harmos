/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod registration;

/* === Re-Exports === */
pub use registration::ObserverRegistration;

/* === Imports === */
// Standard Library Imports
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::{
    ObserverBackend, ObserverDefinition, ObserverInstanceId, ObserverKey, ObserverPhase,
    ObserverScopeId, ObserverStatus,
};
use crate::supervision::{self, Task};
use crate::{Error, Result};

/* === Constants === */
static NEXT_SCOPE: AtomicU64 = AtomicU64::new(0);

/* === Macros === */

/* === Type Aliases === */
type Outcome = std::result::Result<(), Arc<Error>>;
type Resources = Arc<dyn Any + Send + Sync>;
type Running = Arc<ObserverTask>;
type Completion = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/* === Structs === */
struct ObserverTask {
    task: Task<Outcome>,
    children: Arc<ChildSettlement>,
}

pub(crate) struct ChildSettlement {
    pending: Mutex<Vec<Arc<crate::supervision::instance::Instance>>>,
    lifecycle: Arc<supervision::instance::Instance>,
    outcome: Mutex<Option<Outcome>>,
    backend_returned: AtomicBool,
    backend_cleanup: AtomicBool,
    owner: Weak<Owner>,
    key: ObserverKey,
    instance_id: ObserverInstanceId,
}

#[derive(Clone)]
pub struct ObserverManager {
    inner: Arc<Owner>,
}

pub struct ObserverScope<R> {
    scope: Arc<Scope>,
    marker: PhantomData<fn() -> R>,
}

pub(crate) struct ObserverRetirement {
    outcome: Outcome,
    _scope: Arc<Scope>,
    _tasks: Vec<Running>,
    _definitions: Vec<Arc<Registration>>,
}

#[must_use = "an observer lease holds activation demand"]
pub struct ObserverLease {
    owner: Weak<Owner>,
    key: ObserverKey,
    instance_id: ObserverInstanceId,
    demand_id: u64,
    task: Running,
    released: bool,
}

#[derive(Clone)]
pub struct ObserverInstance {
    task: Running,
}

pub struct ObserverActivation {
    owner: Weak<Owner>,
    key: ObserverKey,
    instance_id: ObserverInstanceId,
    stopping: watch::Receiver<bool>,
    children: Arc<ChildSettlement>,
}

struct Owner {
    registry: Mutex<Registry>,
    statuses: watch::Sender<BTreeMap<ObserverKey, ObserverStatus>>,
}

struct Scope {
    owner: Weak<Owner>,
    id: ObserverScopeId,
    resources: Resources,
    closed: AtomicBool,
    retirement: Mutex<Option<Outcome>>,
}

struct Registry {
    closed: bool,
    retired_failure: Option<Arc<Error>>,
    next: u64,
    definitions: BTreeMap<String, Arc<Registration>>,
    scopes: BTreeMap<ObserverScopeId, Arc<Scope>>,
    instances: BTreeMap<ObserverKey, Instance>,
}

struct Registration {
    definition: ObserverDefinition,
    backend: Arc<dyn Backend>,
    closing: AtomicBool,
    scope: Option<ObserverScopeId>,
    published: Arc<AtomicBool>,
    #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
    plugin: Option<crate::load::PluginScope>,
}

struct Instance {
    status: ObserverStatus,
    leases: BTreeSet<u64>,
    task: Running,
}

/* === Enums === */

/* === Traits === */
trait Backend: Send + Sync {
    fn resources(&self) -> TypeId;
    fn run(self: Arc<Self>, resources: Resources, activation: ObserverActivation) -> Completion;
}

/* === Implementations === */
impl<B: ObserverBackend> Backend for B {
    fn resources(&self) -> TypeId {
        TypeId::of::<B::Resources>()
    }

    fn run(self: Arc<Self>, resources: Resources, activation: ObserverActivation) -> Completion {
        Box::pin(async move {
            let resources =
                resources
                    .downcast::<B::Resources>()
                    .map_err(|_| Error::InvalidInput {
                        field: "observer scope",
                        reason: "resource type differs from the registered backend",
                    })?;
            ObserverBackend::run(self.as_ref(), resources, activation).await
        })
    }
}

impl Registration {
    fn is_published(&self) -> bool {
        if !self.published.load(Ordering::Acquire) {
            return false;
        }
        #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
        if self.plugin.as_ref().is_some_and(|scope| !scope.is_ready()) {
            return false;
        }
        true
    }
}

impl ObserverManager {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Owner {
                registry: Mutex::new(Registry {
                    closed: false,
                    retired_failure: None,
                    next: 0,
                    definitions: BTreeMap::new(),
                    scopes: BTreeMap::new(),
                    instances: BTreeMap::new(),
                }),
                statuses: watch::channel(BTreeMap::new()).0,
            }),
        }
    }

    pub fn register<B: ObserverBackend>(&self, backend: B) -> Result<()> {
        let definition = backend.definition();
        validate(&definition)?;
        let registration = Arc::new(Registration {
            definition,
            backend: Arc::new(backend),
            closing: AtomicBool::new(false),
            scope: None,
            published: Arc::new(AtomicBool::new(true)),
            #[cfg(any(feature = "load", feature = "guest-host", feature = "sidecar-host"))]
            plugin: None,
        });
        let mut registry = self
            .inner
            .registry
            .lock()
            .expect("observer registry is healthy");
        registry.admit()?;
        let id = registration.definition.id.clone();
        if registry.definitions.contains_key(&id) {
            return Err(Error::AlreadyRegistered {
                domain: "observer",
                name: id,
            });
        }
        registry.definitions.insert(id, registration);
        Ok(())
    }

    pub fn definition(&self, id: &str) -> Option<ObserverDefinition> {
        self.inner
            .registry
            .lock()
            .expect("observer registry is healthy")
            .definitions
            .get(id)
            .filter(|item| item.is_published())
            .map(|item| item.definition.clone())
    }

    pub fn definitions(&self) -> Vec<ObserverDefinition> {
        self.inner
            .registry
            .lock()
            .expect("observer registry is healthy")
            .definitions
            .values()
            .filter(|item| item.is_published())
            .map(|item| item.definition.clone())
            .collect()
    }

    pub fn scope<R: Send + Sync + 'static>(&self, resources: Arc<R>) -> Result<ObserverScope<R>> {
        let mut registry = self
            .inner
            .registry
            .lock()
            .expect("observer registry is healthy");
        registry.admit()?;
        let id = ObserverScopeId(
            NEXT_SCOPE
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map_err(|_| Error::InvalidInput {
                    field: "observer scope",
                    reason: "identity space exhausted",
                })?,
        );
        let scope = Arc::new(Scope {
            owner: Arc::downgrade(&self.inner),
            id,
            resources,
            closed: AtomicBool::new(false),
            retirement: Mutex::new(None),
        });
        registry.scopes.insert(id, scope.clone());
        Ok(ObserverScope {
            scope,
            marker: PhantomData,
        })
    }

    pub async fn activate<R: Send + Sync + 'static>(
        &self,
        id: &str,
        scope: &ObserverScope<R>,
    ) -> Result<ObserverLease> {
        self.check_scope(&scope.scope)?;
        let key = ObserverKey {
            definition: id.to_owned(),
            scope: scope.id(),
        };
        let mut joined: Option<(ObserverInstanceId, Running)> = None;
        loop {
            let waiting =
                {
                    let mut registry = self
                        .inner
                        .registry
                        .lock()
                        .expect("observer registry is healthy");
                    registry.admit()?;
                    if scope.scope.closed.load(Ordering::Acquire) {
                        return Err(Error::Closing {
                            domain: "observer scope",
                        });
                    }
                    let registration = registry.definitions.get(id).cloned().ok_or_else(|| {
                        Error::NotRegistered {
                            domain: "observer",
                            name: id.to_owned(),
                        }
                    })?;
                    if !registration.is_published() {
                        return Err(Error::NotReady {
                            domain: "observer catalog",
                        });
                    }
                    if registration.scope.is_some_and(|owner| owner != scope.id()) {
                        return Err(Error::InvalidInput {
                            field: "observer scope",
                            reason: "definition belongs to another resource scope",
                        });
                    }
                    if registration.closing.load(Ordering::Acquire) {
                        return Err(Error::Closing {
                            domain: "observer definition",
                        });
                    }
                    if registration.backend.resources() != TypeId::of::<R>() {
                        return Err(Error::InvalidInput {
                            field: "observer scope",
                            reason: "resource type differs from the registered backend",
                        });
                    }
                    let lease = registry.next()?;
                    if let Some(instance) = registry.instances.get_mut(&key)
                        && matches!(
                            instance.status.phase,
                            ObserverPhase::Starting | ObserverPhase::Running
                        )
                    {
                        instance.leases.insert(lease);
                        instance.status.leases = instance.leases.len();
                        let held = self.lease(&key, instance, lease);
                        self.inner.publish(&registry);
                        return Ok(held);
                    }
                    if let Some(instance) = registry.instances.get(&key)
                        && joined
                            .as_ref()
                            .is_none_or(|(instance_id, _)| *instance_id != instance.status.id)
                    {
                        Some((instance.status.id, instance.task.clone()))
                    } else {
                        let instance_id = ObserverInstanceId(registry.next()?);
                        let owner = Arc::downgrade(&self.inner);
                        let activation_key = key.clone();
                        let resources = scope.scope.resources.clone();
                        let stopping = watch::channel(false).0;
                        let lifecycle =
                            Arc::new(supervision::instance::Instance::queued(stopping.clone()));
                        let children = Arc::new(ChildSettlement {
                            pending: Mutex::new(Vec::new()),
                            lifecycle,
                            outcome: Mutex::new(None),
                            backend_returned: AtomicBool::new(false),
                            backend_cleanup: AtomicBool::new(false),
                            owner: owner.clone(),
                            key: activation_key.clone(),
                            instance_id,
                        });
                        let completion = Arc::clone(&children);
                        let task = Task::with_cancellation(stopping, move |stopping| async move {
                            let admitted = completion.lifecycle.initialize_attempt(0);
                            let activation = ObserverActivation {
                                owner: owner.clone(),
                                key: activation_key.clone(),
                                instance_id,
                                stopping: stopping.subscribe(),
                                children: Arc::clone(&completion),
                            };
                            // Calling backend code happens inside the owned attempt,
                            // including constructing its future, so panics are joined.
                            let outcome = supervision::joined(async move {
                                if !admitted {
                                    return Ok(());
                                }
                                registration
                                    .backend
                                    .clone()
                                    .run(resources, activation)
                                    .await
                            })
                            .await;
                            let outcome = match outcome {
                                Ok(outcome) => {
                                    completion.backend_returned.store(true, Ordering::Release);
                                    outcome.map_err(Arc::new)
                                }
                                Err(source) => Err(Arc::new(Error::TaskFailed {
                                    domain: "observer backend",
                                    source: Arc::new(source),
                                })),
                            };
                            *completion.outcome.lock().unwrap() = Some(outcome.clone());
                            if !completion.backend_returned.load(Ordering::Acquire)
                                && let Some(owner) = owner.upgrade()
                            {
                                owner.complete(&activation_key, instance_id, &outcome);
                            }
                            let outcome = completion.finish(false).await;
                            if let Some(owner) = owner.upgrade() {
                                owner.complete(&activation_key, instance_id, &outcome);
                            }
                            outcome
                        });
                        let task = Arc::new(ObserverTask { task, children });
                        let instance = Instance {
                            status: ObserverStatus {
                                id: instance_id,
                                phase: ObserverPhase::Starting,
                                leases: 1,
                                failure: None,
                            },
                            leases: BTreeSet::from([lease]),
                            task,
                        };
                        let held = self.lease(&key, &instance, lease);
                        registry.instances.insert(key.clone(), instance);
                        self.inner.publish(&registry);
                        return Ok(held);
                    }
                };
            if let Some((instance_id, task)) = waiting {
                // Retain the joined activation until its replacement is admitted
                // in the same critical section. Shutdown must never see a gap
                // that forgets a failure while reactivation is still pending.
                let result = settle(vec![task.clone()]).await;
                if !task.children.lifecycle.is_settled() {
                    return Err(result.err().unwrap_or(Error::NotReady {
                        domain: "observer cleanup",
                    }));
                }
                joined = Some((instance_id, task));
            }
        }
    }

    fn lease(&self, key: &ObserverKey, instance: &Instance, id: u64) -> ObserverLease {
        ObserverLease {
            owner: Arc::downgrade(&self.inner),
            key: key.clone(),
            instance_id: instance.status.id,
            demand_id: id,
            task: instance.task.clone(),
            released: false,
        }
    }

    pub fn status(&self, key: &ObserverKey) -> Option<ObserverStatus> {
        self.inner.statuses.borrow().get(key).cloned()
    }

    pub fn statuses(&self) -> BTreeMap<ObserverKey, ObserverStatus> {
        self.inner.statuses.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<BTreeMap<ObserverKey, ObserverStatus>> {
        self.inner.statuses.subscribe()
    }

    pub async fn stop_scope<R>(&self, scope: &ObserverScope<R>) -> Result<()> {
        self.retire_scope(scope)
            .await?
            .into_outcome()
            .map_err(|source| Error::TaskFailed {
                domain: "observer",
                source,
            })
    }

    pub(crate) async fn retire_scope<R>(
        &self,
        scope: &ObserverScope<R>,
    ) -> Result<ObserverRetirement> {
        self.check_scope(&scope.scope)?;
        if let Some(outcome) = scope.scope.retirement.lock().unwrap().clone() {
            return Ok(ObserverRetirement {
                outcome,
                _scope: scope.scope.clone(),
                _tasks: Vec::new(),
                _definitions: Vec::new(),
            });
        }
        let (tasks, definitions) = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .expect("observer registry is healthy");
            scope.scope.closed.store(true, Ordering::Release);
            let definitions: Vec<_> = registry
                .definitions
                .values()
                .filter(|entry| entry.scope == Some(scope.id()))
                .cloned()
                .collect();
            for definition in &definitions {
                definition.closing.store(true, Ordering::Release);
            }
            let tasks = registry.stop(|key| key.scope == scope.id());
            self.inner.publish(&registry);
            (tasks, definitions)
        };
        let outcome = settle(tasks.clone()).await.map_err(Arc::new);
        let original_scope = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .expect("observer registry is healthy");
            if registry.instances.iter().any(|(key, instance)| {
                key.scope == scope.id() && !instance.task.children.lifecycle.is_settled()
            }) {
                return Err(match &outcome {
                    Err(source) => Error::TaskFailed {
                        domain: "observer cleanup",
                        source: source.clone(),
                    },
                    Ok(()) => Error::NotReady {
                        domain: "observer cleanup",
                    },
                });
            }
            if let Err(source) = &outcome {
                registry
                    .retired_failure
                    .get_or_insert_with(|| Arc::clone(source));
            }
            let original_scope = registry
                .scopes
                .remove(&scope.id())
                .unwrap_or_else(|| scope.scope.clone());
            registry.instances.retain(|key, _| key.scope != scope.id());
            for definition in &definitions {
                if registry
                    .definitions
                    .get(&definition.definition.id)
                    .is_some_and(|current| Arc::ptr_eq(current, definition))
                {
                    registry.definitions.remove(&definition.definition.id);
                }
            }
            self.inner.publish(&registry);
            original_scope
        };
        let outcome = scope
            .scope
            .retirement
            .lock()
            .unwrap()
            .get_or_insert(outcome)
            .clone();
        Ok(ObserverRetirement {
            outcome,
            _scope: original_scope,
            _tasks: tasks,
            _definitions: definitions,
        })
    }

    pub async fn unregister(&self, id: &str) -> Result<()> {
        let (registration, tasks) = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .expect("observer registry is healthy");
            let registration =
                registry
                    .definitions
                    .get(id)
                    .cloned()
                    .ok_or_else(|| Error::NotRegistered {
                        domain: "observer",
                        name: id.to_owned(),
                    })?;
            registration.closing.store(true, Ordering::Release);
            let tasks = registry.stop(|key| key.definition == id);
            self.inner.publish(&registry);
            (registration, tasks)
        };
        let result = settle(tasks.clone()).await;
        let mut registry = self
            .inner
            .registry
            .lock()
            .expect("observer registry is healthy");
        if registry.instances.iter().any(|(key, instance)| {
            key.definition == id && !instance.task.children.lifecycle.is_settled()
        }) {
            return result;
        }
        if registry
            .definitions
            .get(id)
            .is_some_and(|current| Arc::ptr_eq(current, &registration))
        {
            registry.definitions.remove(id);
            registry.instances.retain(|key, _| key.definition != id);
            self.inner.publish(&registry);
        }
        result
    }

    pub(crate) fn cleanup_pending(&self) -> bool {
        self.inner
            .registry
            .lock()
            .unwrap()
            .instances
            .values()
            .any(|instance| !instance.task.children.lifecycle.is_settled())
    }

    pub(crate) fn close_admission(&self) {
        let scopes = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .expect("observer registry is healthy");
            registry.closed = true;
            for scope in registry.scopes.values() {
                scope.closed.store(true, Ordering::Release);
            }
            registry.stop(|_| true);
            self.inner.publish(&registry);
            registry.release_scopes()
        };
        // Releasing the final resource reference can invoke author Drop code.
        drop(scopes);
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.close_admission();
        let tasks = self
            .inner
            .registry
            .lock()
            .expect("observer registry is healthy")
            .instances
            .values()
            .map(|item| item.task.clone())
            .collect();
        let result = settle(tasks).await;
        let (definitions, scopes, retired_failure) = {
            let mut registry = self
                .inner
                .registry
                .lock()
                .expect("observer registry is healthy");
            let ids: Vec<_> = registry
                .definitions
                .keys()
                .filter(|id| {
                    registry
                        .instances
                        .iter()
                        .filter(|(key, _)| key.definition == **id)
                        .all(|(_, instance)| instance.task.children.lifecycle.is_settled())
                })
                .cloned()
                .collect();
            let definitions = ids
                .into_iter()
                .filter_map(|id| registry.definitions.remove(&id))
                .collect::<Vec<_>>();
            (
                definitions,
                registry.release_scopes(),
                registry.retired_failure.clone(),
            )
        };
        // Capability factories may retain runtime handles. Drop them outside
        // the registry lock after every active use has joined.
        drop((definitions, scopes));
        result?;
        match retired_failure {
            Some(source) => Err(Error::TaskFailed {
                domain: "retired observer",
                source,
            }),
            None => Ok(()),
        }
    }

    fn check_scope(&self, scope: &Scope) -> Result<()> {
        if !Weak::ptr_eq(&scope.owner, &Arc::downgrade(&self.inner)) {
            return Err(Error::InvalidInput {
                field: "observer scope",
                reason: "scope belongs to another manager",
            });
        }
        Ok(())
    }
}

impl Registry {
    fn release_scopes(&mut self) -> Vec<Arc<Scope>> {
        let ids: Vec<_> = self
            .scopes
            .keys()
            .filter(|id| {
                self.instances
                    .iter()
                    .filter(|(key, _)| key.scope == **id)
                    .all(|(_, instance)| instance.task.children.lifecycle.is_settled())
            })
            .copied()
            .collect();
        ids.into_iter()
            .filter_map(|id| self.scopes.remove(&id))
            .collect()
    }

    fn admit(&self) -> Result<()> {
        if self.closed {
            Err(Error::Closing { domain: "observer" })
        } else {
            Ok(())
        }
    }

    fn next(&mut self) -> Result<u64> {
        self.next = self.next.checked_add(1).ok_or(Error::InvalidInput {
            field: "observer identity",
            reason: "identity space exhausted",
        })?;
        Ok(self.next)
    }

    fn stop(&mut self, matches: impl Fn(&ObserverKey) -> bool) -> Vec<Running> {
        self.instances
            .iter_mut()
            .filter(|(key, _)| matches(key))
            .map(|(_, item)| {
                item.leases.clear();
                item.status.leases = 0;
                item.stop();
                item.task.clone()
            })
            .collect()
    }
}

impl ChildSettlement {
    #[cfg(any(feature = "sidecar", feature = "sidecar-host"))]
    pub(crate) async fn cleanup<F: std::future::Future>(
        &self,
        status: &mut tokio::sync::watch::Receiver<harmos_contract::observers::State>,
        closing: F,
    ) -> F::Output {
        tokio::pin!(closing);
        loop {
            self.cleanup_phase(*status.borrow_and_update());
            tokio::select! {
                result = &mut closing => {
                    self.cleanup_phase(*status.borrow_and_update());
                    return result;
                }
                changed = status.changed() => {
                    if changed.is_err() {
                        return closing.await;
                    }
                }
            }
        }
    }

    #[cfg(any(feature = "sidecar", feature = "sidecar-host"))]
    pub(crate) fn cleanup_phase(&self, status: harmos_contract::observers::State) {
        use harmos_contract::observers::{Exit, State};
        use supervision::instance::Phase;
        let phase = match status {
            State::Stopping | State::Retained(Exit::StopFailed | Exit::StopPanicked) => {
                Phase::Stopping
            }
            State::Finalizing | State::Retained(Exit::FinalizeFailed | Exit::FinalizePanicked) => {
                Phase::Finalizing
            }
            _ => return,
        };
        self.lifecycle.transition(phase);
    }

    #[cfg(feature = "sidecar-host")]
    pub(crate) async fn settle_controls(&self) -> Result<()> {
        let pending = self.pending.lock().unwrap().clone();
        for control in pending {
            control
                .settle()
                .await
                .map_err(|source| Error::OperationSettlement { source })?;
        }
        Ok(())
    }

    #[cfg(feature = "sidecar-host")]
    pub(crate) fn submit(
        &self,
        submit: impl FnOnce() -> Result<crate::operations::EncodedInvocation>,
    ) -> Result<crate::operations::EncodedInvocation> {
        self.submit_control(false, submit)
    }

    #[cfg(feature = "sidecar-host")]
    pub(crate) fn submit_control(
        &self,
        retirement: bool,
        submit: impl FnOnce() -> Result<crate::operations::EncodedInvocation>,
    ) -> Result<crate::operations::EncodedInvocation> {
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|control| !control.is_settled());
        if pending.len() >= 64 || (!retirement && self.outcome.lock().unwrap().is_some()) {
            return Err(Error::ResourceExhausted {
                resource: "observer physical children",
            });
        }
        let invocation = submit()?;
        pending.push(Arc::clone(&invocation.control));
        Ok(invocation)
    }

    async fn finish(&self, recover: bool) -> Outcome {
        let outcome = self
            .outcome
            .lock()
            .unwrap()
            .as_ref()
            .expect("backend outcome recorded before child settlement")
            .clone();
        if self.lifecycle.status().outcome().is_none() {
            let terminal = match &outcome {
                Ok(()) if self.lifecycle.status().cancellation_requested() => {
                    supervision::instance::Outcome::Cancelled
                }
                Ok(()) => supervision::instance::Outcome::Succeeded,
                Err(_) => supervision::instance::Outcome::Failed,
            };
            self.lifecycle.complete(terminal);
        }
        if !self.backend_cleanup.load(Ordering::Acquire) {
            self.lifecycle
                .transition(supervision::instance::Phase::Finalizing);
        }
        let pending = self.pending.lock().unwrap().clone();
        if !self.backend_returned.load(Ordering::Acquire) {
            for control in &pending {
                control.cancel();
            }
        }
        let joining = pending.into_iter().map(|control| async move {
            control
                .settle()
                .await
                .map_err(|source| Arc::new(Error::OperationSettlement { source }))
        });
        if let Err(failure) = supervision::settle_all(joining).await {
            self.lifecycle.interrupted(failure.clone());
            return Err(failure);
        }
        self.pending.lock().unwrap().clear();
        if self.backend_returned.load(Ordering::Acquire) {
            if self.backend_cleanup.load(Ordering::Acquire) && !self.lifecycle.is_settled() {
                if !recover {
                    return Err(self
                        .lifecycle
                        .status()
                        .cleanup_failure()
                        .cloned()
                        .expect("retained backend failure"));
                }
                self.lifecycle.settle().await?;
            } else {
                self.lifecycle.finished();
            }
        } else if let Err(failure) = &outcome {
            self.lifecycle.interrupted(failure.clone());
        }
        outcome
    }
}

impl ObserverTask {
    fn stop(&self) {
        self.children.lifecycle.cancel();
    }

    async fn settle(&self) -> std::result::Result<Arc<Outcome>, Arc<tokio::task::JoinError>> {
        let children = Arc::clone(&self.children);
        self.task
            .settle_retry(move || {
                let retry =
                    children.outcome.lock().unwrap().is_some() && !children.lifecycle.is_settled();
                retry.then_some(async move {
                    let outcome = children.finish(true).await;
                    if let Some(owner) = children.owner.upgrade() {
                        owner.complete(&children.key, children.instance_id, &outcome);
                    }
                    outcome
                })
            })
            .await
    }
}

impl Instance {
    fn stop(&mut self) {
        if self.status.phase != ObserverPhase::Stopped {
            self.status.phase = ObserverPhase::Stopping;
        }
        self.task.stop();
    }
}

impl Owner {
    fn publish(&self, registry: &Registry) {
        self.statuses.send_replace(
            registry
                .instances
                .iter()
                .map(|(key, item)| (key.clone(), item.status.clone()))
                .collect(),
        );
    }

    fn release(&self, key: &ObserverKey, instance_id: ObserverInstanceId, id: u64) -> bool {
        let mut registry = self.registry.lock().expect("observer registry is healthy");
        let Some(item) = registry
            .instances
            .get_mut(key)
            .filter(|item| item.status.id == instance_id)
        else {
            return true;
        };
        item.leases.remove(&id);
        item.status.leases = item.leases.len();
        let final_release = item.leases.is_empty();
        if final_release {
            item.stop();
        }
        self.publish(&registry);
        final_release
    }

    fn complete(&self, key: &ObserverKey, instance_id: ObserverInstanceId, outcome: &Outcome) {
        let mut registry = self.registry.lock().expect("observer registry is healthy");
        if let Some(item) = registry
            .instances
            .get_mut(key)
            .filter(|item| item.status.id == instance_id)
        {
            item.status.phase = if item.task.children.lifecycle.is_settled() {
                ObserverPhase::Stopped
            } else {
                ObserverPhase::Stopping
            };
            item.status.failure = outcome.as_ref().err().cloned();
            self.publish(&registry);
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        let registry = self
            .registry
            .get_mut()
            .expect("observer registry is healthy");
        registry.stop(|_| true);
    }
}

impl ObserverRetirement {
    pub(crate) fn into_outcome(self) -> Outcome {
        self.outcome
    }
}

impl<R> ObserverScope<R> {
    pub fn id(&self) -> ObserverScopeId {
        self.scope.id
    }
}

impl<R> Clone for ObserverScope<R> {
    fn clone(&self) -> Self {
        Self {
            scope: self.scope.clone(),
            marker: PhantomData,
        }
    }
}

impl ObserverInstance {
    pub fn id(&self) -> ObserverInstanceId {
        self.task.children.instance_id
    }

    pub fn key(&self) -> &ObserverKey {
        &self.task.children.key
    }

    pub fn is_settled(&self) -> bool {
        self.task.children.lifecycle.is_settled()
    }

    pub fn lifecycle(&self) -> super::ObserverLifecycle {
        self.task.children.lifecycle.status()
    }

    pub fn subscribe_lifecycle(&self) -> watch::Receiver<super::ObserverLifecycle> {
        self.task.children.lifecycle.settlement()
    }

    pub fn cancel(&self) {
        let children = &self.task.children;
        if let Some(owner) = children.owner.upgrade() {
            let mut registry = owner.registry.lock().expect("observer registry is healthy");
            if let Some(instance) = registry.instances.get_mut(&children.key)
                && instance.status.id == children.instance_id
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

impl ObserverLease {
    pub fn instance(&self) -> ObserverInstance {
        ObserverInstance {
            task: self.task.clone(),
        }
    }

    pub fn key(&self) -> &ObserverKey {
        &self.key
    }
    pub fn id(&self) -> ObserverInstanceId {
        self.instance_id
    }

    pub async fn release(mut self) -> Result<()> {
        let final_release = self.request();
        if final_release {
            settle(vec![self.task.clone()]).await
        } else {
            Ok(())
        }
    }

    fn request(&mut self) -> bool {
        if self.released {
            return false;
        }
        self.released = true;
        if let Some(owner) = self.owner.upgrade() {
            owner.release(&self.key, self.instance_id, self.demand_id)
        } else {
            self.task.stop();
            true
        }
    }
}

impl Drop for ObserverLease {
    fn drop(&mut self) {
        self.request();
    }
}

impl ObserverActivation {
    #[cfg(any(feature = "sidecar", feature = "sidecar-host"))]
    pub(crate) fn retain<R: supervision::cleanup::Resource>(
        &self,
        resource: R,
        failure: Arc<Error>,
    ) {
        self.children.backend_cleanup.store(true, Ordering::Release);
        self.children.lifecycle.retain(resource, failure);
    }

    #[cfg(feature = "sidecar-host")]
    pub(crate) fn depend_on(&self, provider: &Arc<supervision::instance::Instance>) -> Result<()> {
        self.children.lifecycle.depend_on(provider, false)
    }

    #[cfg(any(feature = "sidecar", feature = "sidecar-host"))]
    pub(crate) fn child_settlement(&self) -> Arc<ChildSettlement> {
        Arc::clone(&self.children)
    }

    pub fn key(&self) -> &ObserverKey {
        &self.key
    }
    pub fn id(&self) -> ObserverInstanceId {
        self.children.instance_id
    }

    pub fn ready(&self) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut registry = owner.registry.lock().expect("observer registry is healthy");
        if let Some(item) = registry.instances.get_mut(&self.key).filter(|item| {
            item.status.id == self.instance_id && item.status.phase == ObserverPhase::Starting
        }) {
            item.status.phase = ObserverPhase::Running;
            item.task
                .children
                .lifecycle
                .transition(supervision::instance::Phase::Running);
            owner.publish(&registry);
        }
    }

    pub fn begin_stopping(&self) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut registry = owner.registry.lock().expect("observer registry is healthy");
        if let Some(item) = registry.instances.get_mut(&self.key).filter(|item| {
            item.status.id == self.instance_id && item.status.phase != ObserverPhase::Stopped
        }) {
            item.status.phase = ObserverPhase::Stopping;
            item.task
                .children
                .lifecycle
                .transition(supervision::instance::Phase::Stopping);
            owner.publish(&registry);
        }
    }

    pub async fn stopping(&mut self) {
        while !*self.stopping.borrow_and_update() {
            if self.stopping.changed().await.is_err() {
                break;
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn settle(tasks: Vec<Running>) -> Result<()> {
    let pending = tasks.into_iter().map(|task| async move {
        match task.settle().await {
            Ok(outcome) => outcome
                .as_ref()
                .clone()
                .map_err(|source| Error::TaskFailed {
                    domain: "observer",
                    source,
                }),
            Err(source) => Err(Error::TaskFailed {
                domain: "observer supervisor",
                source,
            }),
        }
    });
    supervision::settle_all(pending).await
}

fn validate(definition: &ObserverDefinition) -> Result<()> {
    if definition.id.trim().is_empty() || definition.id.chars().any(char::is_control) {
        return Err(Error::InvalidInput {
            field: "observer identity",
            reason: "must be nonempty and contain no control characters",
        });
    }
    if definition.interval.is_zero() {
        return Err(Error::InvalidInput {
            field: "observer interval",
            reason: "must be positive",
        });
    }
    let mut names = BTreeSet::new();
    for stream in &definition.streams {
        if stream.id.is_empty()
            || stream.schema == 0
            || stream.window == 0
            || !names.insert(stream.id.as_str())
        {
            return Err(Error::InvalidInput {
                field: "observer streams",
                reason: "stream identities must be unique and schema/window must be positive",
            });
        }
    }
    for key in &definition.demand_keys {
        if key.is_empty() || !names.insert(key.as_str()) {
            return Err(Error::InvalidInput {
                field: "observer demand",
                reason: "demand keys must be nonempty, unique, and distinct from streams",
            });
        }
    }
    Ok(())
}

/* === Tests === */
