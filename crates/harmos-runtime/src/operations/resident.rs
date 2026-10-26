/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resident declarations, observers and services, run as instances of the
// operation manager. One registry owns their definitions, the resources each
// activation shares, the instances keyed by definition and resources, and the
// leases that keep an instance running. A kind contributes only its driver:
// observers tick at their declared interval, services run until stopped under
// their restart policy.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::caller::Identity;
use super::instances::Instances;
use super::{OperationGeneration, OperationInstanceId, OperationStatus};
use crate::observers::ObserverDefinition;
use crate::services::{ServiceDefinition, ServiceStatus};
use crate::supervision::{self, Task, instance::Instance, instance::Phase};
use crate::{Error, Result};

/* === Constants === */

static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

const ROOT: ScopeId = ScopeId(0);

/* === Macros === */

/* === Type Aliases === */

pub(crate) type Outcome = std::result::Result<(), Arc<Error>>;
pub(crate) type Completion = Pin<Box<dyn Future<Output = Outcome> + Send>>;
pub(crate) type Resources = Arc<dyn Any + Send + Sync>;

/* === Structs === */

// The declaration a resident handle names when its definition is selected by
// identity at runtime. It is never registered by its own identity.
pub struct Activation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeId(u64);

// One resident instance per definition and shared resources.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub definition: String,
    pub scope: ScopeId,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub instance: OperationInstanceId,
    pub kind: Kind,
    pub leases: usize,
    pub lifecycle: OperationStatus,
    // Restart credit and deactivation history; services only.
    pub health: Option<ServiceStatus>,
    pub failure: Option<Arc<Error>>,
}

#[derive(Clone)]
pub(crate) struct Registry {
    shared: Arc<Shared>,
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
#[derive(Clone)]
pub(crate) struct WeakRegistry(Weak<Shared>);

pub(crate) struct Shared {
    identity: Arc<()>,
    root_resources: Arc<()>,
    index: Weak<Instances>,
    root: Arc<Scope>,
    state: Mutex<State>,
    statuses: watch::Sender<BTreeMap<Key, Snapshot>>,
    assembly: Mutex<Vec<Lease>>,
}

pub(crate) struct Scope {
    owner: Weak<()>,
    id: ScopeId,
    address: usize,
    resources: Resources,
    closed: AtomicBool,
    retirement: Mutex<Option<Outcome>>,
    detached: Mutex<Vec<Lease>>,
}

pub(crate) struct Registration {
    pub(crate) definition: Definition,
    runner: Arc<dyn Runner>,
    generation: OperationGeneration,
    closing: AtomicBool,
    scope: Option<ScopeId>,
    internal: bool,
    published: Arc<AtomicBool>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    plugin: Option<crate::plugin::PluginScope>,
}

// Retains a retired scope's resources and instances until its caller has
// consumed the outcome, so author Drop code never runs under a registry lock.
pub(crate) struct Retirement {
    outcome: Outcome,
    _scope: Arc<Scope>,
    _running: Vec<Running>,
    _definitions: Vec<Arc<Registration>>,
}

// A staged set of definitions that become visible when committed and are
// withdrawn when dropped uncommitted.
#[must_use = "commit the resident catalog or drop it to release its reservations"]
pub struct Staged {
    owner: Weak<Shared>,
    scope: Arc<Scope>,
    entries: Vec<Arc<Registration>>,
    published: Arc<AtomicBool>,
}

// What a runner receives to start one instance.
pub(crate) struct Start {
    pub(crate) resources: Resources,
    pub(crate) lifecycle: Arc<Instance>,
    pub(crate) stopping: watch::Sender<bool>,
    pub(crate) report: Report,
    pub(crate) operations: super::Manager,
}

#[derive(Clone)]
pub(crate) struct Running {
    task: Arc<Task<Outcome>>,
    settlement: Arc<dyn Settlement>,
    report: Report,
}

// Progress a kind's driver reports for its own instance.
#[derive(Clone)]
pub(crate) struct Report {
    owner: Weak<Shared>,
    key: Key,
    id: OperationInstanceId,
    kind: Kind,
}

#[must_use = "a resident lease holds activation demand"]
pub(crate) struct Lease {
    running: Running,
    demand: u64,
    generation: OperationGeneration,
    released: bool,
    joins_release: bool,
}

struct State {
    closed: bool,
    retired_failure: Option<Arc<Error>>,
    next: u64,
    reserved: BTreeSet<String>,
    definitions: BTreeMap<String, Arc<Registration>>,
    scopes: BTreeMap<ScopeId, Arc<Scope>>,
    addresses: BTreeMap<usize, ScopeId>,
    instances: BTreeMap<Key, Entry>,
}

struct Entry {
    snapshot: Snapshot,
    internal: bool,
    leases: BTreeSet<u64>,
    running: Running,
    stopping: bool,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Observer,
    Service,
}

#[derive(Clone, Debug)]
pub enum Definition {
    Observer(ObserverDefinition),
    Service(ServiceDefinition),
}

enum Acquisition {
    Lease(Lease),
    Wait(OperationInstanceId, Running),
}

/* === Traits === */

// A kind's driver. It spawns one instance over its resources and reports the
// instance's progress; the registry owns everything else.
pub(crate) trait Runner: Send + Sync + 'static {
    fn resources(&self) -> TypeId;

    // A definition whose resident can be created only once is not reactivated
    // after its instance stops.
    fn reactivates(&self) -> bool {
        true
    }

    fn spawn(self: Arc<Self>, start: Start) -> (Task<Outcome>, Arc<dyn Settlement>);
}

// The physical cleanup a kind retains for one instance.
pub(crate) trait Settlement: Send + Sync + 'static {
    fn lifecycle(&self) -> &Arc<Instance>;

    // Record the instance's logical outcome as its task completes.
    fn record(&self, _outcome: &Outcome) {}

    // Retry a cleanup that failed after the task returned; `None` once settled.
    fn retry(self: Arc<Self>, report: Report) -> Option<Completion>;
}

/* === Implementations === */

// The Resident Activation Of A ServiceContext; It Is Never Declared By Its Identity.
impl super::OperationDeclaration for Activation {
    type Request = ();
    type Response = ();
    type Input = super::NoStream;
    type Output = super::NoStream;

    const ID: &'static str = "";
    const POLICY: super::OperationPolicy = super::OperationPolicy {
        timeout: harmos_domain::operations::UNBOUNDED,
        attempt_timeout: harmos_domain::operations::UNBOUNDED,
        retries: super::RetryPolicy::Never,
    };
}

impl Key {
    // The key of `definition`'s instance over the runtime's own resources.
    pub fn root(definition: impl Into<String>) -> Self {
        Self { definition: definition.into(), scope: ROOT }
    }
}

impl Kind {
    pub fn domain(self) -> &'static str {
        match self {
            Self::Observer => "observer",
            Self::Service => "service",
        }
    }

    fn scope(self) -> &'static str {
        match self {
            Self::Observer => "observer scope",
            Self::Service => "service scope",
        }
    }

    fn definition(self) -> &'static str {
        match self {
            Self::Observer => "observer definition",
            Self::Service => "service definition",
        }
    }

    fn catalog(self) -> &'static str {
        match self {
            Self::Observer => "observer catalog",
            Self::Service => "service catalog",
        }
    }
}

impl Definition {
    pub fn id(&self) -> &str {
        match self {
            Self::Observer(definition) => &definition.id,
            Self::Service(definition) => &definition.id,
        }
    }

    pub fn kind(&self) -> Kind {
        match self {
            Self::Observer(_) => Kind::Observer,
            Self::Service(_) => Kind::Service,
        }
    }
}

impl Registration {
    fn is_published(&self) -> bool {
        if !self.published.load(Ordering::Acquire) {
            return false;
        }
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if self.plugin.as_ref().is_some_and(|scope| !scope.is_ready()) {
            return false;
        }
        true
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl WeakRegistry {
    pub(crate) fn upgrade(&self) -> Option<Registry> {
        self.0.upgrade().map(|shared| Registry { shared })
    }
}

impl Registry {
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) fn downgrade(&self) -> WeakRegistry {
        WeakRegistry(Arc::downgrade(&self.shared))
    }

    pub(crate) fn new(index: &Arc<Instances>, reserved: BTreeSet<String>, identity: Arc<()>) -> Self {
        let root_resources = Arc::new(());
        let resources: Resources = Arc::clone(&root_resources) as Resources;
        let root = Arc::new(Scope {
            owner: Arc::downgrade(&identity),
            id: ROOT,
            address: address(&resources),
            resources,
            closed: AtomicBool::new(false),
            retirement: Mutex::new(None),
            detached: Mutex::new(Vec::new()),
        });
        let state = State {
            closed: false,
            retired_failure: None,
            next: 0,
            reserved,
            definitions: BTreeMap::new(),
            scopes: BTreeMap::from([(ROOT, Arc::clone(&root))]),
            addresses: BTreeMap::from([(root.address, ROOT)]),
            instances: BTreeMap::new(),
        };
        Self {
            shared: Arc::new(Shared {
                identity,
                root_resources,
                index: Arc::downgrade(index),
                root,
                state: Mutex::new(state),
                statuses: watch::channel(BTreeMap::new()).0,
                assembly: Mutex::new(Vec::new()),
            }),
        }
    }

    pub(crate) fn identity(&self) -> Arc<()> {
        Arc::clone(&self.shared.identity)
    }

    pub(crate) fn root_resources(&self) -> Arc<()> {
        Arc::clone(&self.shared.root_resources)
    }

    pub(crate) fn root(&self) -> Arc<Scope> {
        Arc::clone(&self.shared.root)
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.shared.state.lock().expect("resident registry is healthy")
    }

    // Register one definition visible at once, optionally bound to one scope.
    pub(crate) fn register(
        &self, definition: Definition, runner: Arc<dyn Runner>, scope: Option<&Arc<Scope>>, internal: bool, reserved: bool,
    ) -> Result<()> {
        validate(&definition)?;
        let registration = Arc::new(Registration {
            definition,
            runner,
            generation: OperationGeneration::fresh()?,
            closing: AtomicBool::new(false),
            scope: scope.map(|scope| scope.id),
            internal,
            published: Arc::new(AtomicBool::new(true)),
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            plugin: None,
        });
        let mut state = self.state();
        state.admit()?;
        let id = registration.definition.id().to_owned();
        if state.definitions.contains_key(&id) || (!reserved && state.reserved.contains(&id)) {
            return Err(Error::AlreadyRegistered { domain: registration.definition.kind().domain(), name: id });
        }
        state.reserved.remove(&id);
        state.definitions.insert(id, registration);
        Ok(())
    }

    // Reserve definitions bound to one scope; they become visible on commit.
    pub(crate) fn stage(
        &self, scope: &Arc<Scope>, entries: Vec<(Definition, Arc<dyn Runner>)>,
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))] plugin: Option<crate::plugin::PluginScope>,
    ) -> Result<Staged> {
        self.check_scope(scope)?;
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if plugin.as_ref().is_some_and(|scope| scope.is_closed()) {
            return Err(Error::Closing { domain: "plugin scope" });
        }
        let published = Arc::new(AtomicBool::new(false));
        let mut staged = Vec::new();
        let mut ids = BTreeSet::new();
        for (definition, runner) in entries {
            validate(&definition)?;
            if !ids.insert(definition.id().to_owned()) {
                return Err(Error::AlreadyRegistered { domain: definition.kind().domain(), name: definition.id().to_owned() });
            }
            staged.push(Arc::new(Registration {
                definition,
                runner,
                generation: OperationGeneration::fresh()?,
                closing: AtomicBool::new(false),
                scope: Some(scope.id),
                internal: false,
                published: Arc::clone(&published),
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                plugin: plugin.clone(),
            }));
        }

        let mut state = self.state();
        state.admit()?;
        if scope.closed.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "resident scope" });
        }
        if let Some(entry) = staged.iter().find(|entry| state.definitions.contains_key(entry.definition.id())) {
            return Err(Error::AlreadyRegistered { domain: entry.definition.kind().domain(), name: entry.definition.id().to_owned() });
        }
        for entry in &staged {
            state.definitions.insert(entry.definition.id().to_owned(), Arc::clone(entry));
        }
        Ok(Staged { owner: Arc::downgrade(&self.shared), scope: Arc::clone(scope), entries: staged, published })
    }

    pub(crate) fn definition(&self, id: &str) -> Option<Definition> {
        self.state().definitions.get(id).filter(|item| item.is_published()).map(|item| item.definition.clone())
    }

    pub(crate) fn definitions(&self) -> Vec<Definition> {
        self.state().definitions.values().filter(|item| item.is_published()).map(|item| item.definition.clone()).collect()
    }

    // The scope that shares these resources, opened on first use.
    pub(crate) fn scope<R: Send + Sync + 'static>(&self, resources: Arc<R>) -> Result<Arc<Scope>> {
        let resources: Resources = resources;
        let address = address(&resources);
        let mut state = self.state();
        state.admit()?;
        if let Some(scope) = state.addresses.get(&address).and_then(|id| state.scopes.get(id)) {
            return Ok(Arc::clone(scope));
        }
        let id = ScopeId(
            NEXT_SCOPE
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| value.checked_add(1))
                .map_err(|_| Error::InvalidInput { field: "resident scope", reason: "identity space exhausted" })?,
        );
        let scope = Arc::new(Scope {
            owner: Arc::downgrade(&self.shared.identity),
            id,
            address,
            resources,
            closed: AtomicBool::new(false),
            retirement: Mutex::new(None),
            detached: Mutex::new(Vec::new()),
        });
        state.scopes.insert(id, Arc::clone(&scope));
        state.addresses.insert(address, id);
        Ok(scope)
    }

    pub(crate) fn scope_of<R: Send + Sync + 'static>(&self, resources: &Arc<R>) -> Option<Arc<Scope>> {
        let address = Arc::as_ptr(resources) as *const () as usize;
        let state = self.state();
        state.addresses.get(&address).and_then(|id| state.scopes.get(id)).cloned()
    }

    // Lease the instance of `id` over `scope`, starting it when none runs.
    // A stopping instance is joined before its replacement starts.
    #[tracing::instrument(target = "harmos::operations", name = "operations.activate", level = "debug", skip_all)]
    pub(super) async fn activate(
        &self, operations: &super::Manager, id: &str, scope: &Arc<Scope>, resources: TypeId, holder: Identity, internal: bool,
    ) -> Result<Lease> {
        let mut joined = None;
        loop {
            match self.acquire(operations, id, scope, resources, joined, internal, &holder)? {
                Acquisition::Lease(lease) => return Ok(lease),
                Acquisition::Wait(instance, running) => {
                    let result = settle(vec![running.clone()]).await;
                    if !running.settlement.lifecycle().is_settled() {
                        return Err(result.err().unwrap_or(Error::NotReady { domain: "resident cleanup" }));
                    }
                    joined = Some(instance);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(target = "harmos::operations", name = "operations.acquire", level = "debug", skip_all)]
    fn acquire(
        &self, operations: &super::Manager, id: &str, scope: &Arc<Scope>, resources: TypeId, joined: Option<OperationInstanceId>,
        internal: bool, holder: &Identity,
    ) -> Result<Acquisition> {
        self.check_scope(scope)?;
        let mut state = self.state();
        let Some(registration) = state.definitions.get(id).cloned() else {
            state.admit()?;
            return Err(Error::NotRegistered { domain: "resident", name: id.to_owned() });
        };
        let kind = registration.definition.kind();
        if state.closed {
            return Err(Error::Closing { domain: kind.domain() });
        }
        if scope.closed.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: kind.scope() });
        }
        if registration.internal && !internal {
            return Err(Error::InvalidInput { field: kind.definition(), reason: "infrastructure demand belongs to its runtime owner" });
        }
        if !registration.is_published() {
            return Err(Error::NotReady { domain: kind.catalog() });
        }
        if registration.closing.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: kind.definition() });
        }
        if registration.scope.is_some_and(|owner| owner != scope.id) || registration.runner.resources() != resources {
            return Err(Error::InvalidInput { field: kind.scope(), reason: "definition belongs to another resource scope or type" });
        }
        let key = Key { definition: id.to_owned(), scope: scope.id };
        let demand = state.next()?;
        if let Some(entry) = state.instances.get_mut(&key) {
            let lifecycle = entry.running.settlement.lifecycle().status();
            if !entry.stopping && lifecycle.outcome().is_none() && !lifecycle.phase().is_terminal() {
                entry.leases.insert(demand);
                entry.snapshot.leases = entry.leases.len();
                let instance = entry.snapshot.instance;
                let lease = Lease::new(entry.running.clone(), demand, registration.generation);
                self.track(instance, entry.running.settlement.lifecycle(), holder);
                self.shared.publish(&state);
                return Ok(Acquisition::Lease(lease));
            }
            if joined != Some(entry.snapshot.instance) {
                return Ok(Acquisition::Wait(entry.snapshot.instance, entry.running.clone()));
            }
            if !registration.runner.reactivates() {
                return Err(Error::NotReady { domain: "retired assembly service" });
            }
        }

        // Start One Instance Over The Shared Resources
        let instance = super::handler::fresh_instance()?;
        let stopping = watch::channel(false).0;
        let index = self.shared.index.clone();
        let lifecycle = Arc::new(Instance::queued(stopping.clone()).resident().on_settled(move || {
            if let Some(index) = index.upgrade() {
                index.forget_resident(instance);
            }
        }));
        lifecycle.trace().record("instance_id", instance.value());
        lifecycle.trace().record("operation", tracing::field::display(registration.definition.id()));
        lifecycle.trace().record("generation", tracing::field::debug(registration.generation));
        let _trace = lifecycle.trace().entered();
        let report = Report { owner: Arc::downgrade(&self.shared), key: key.clone(), id: instance, kind };
        let (task, settlement) = Arc::clone(&registration.runner).spawn(Start {
            resources: Arc::clone(&scope.resources),
            lifecycle: Arc::clone(&lifecycle),
            stopping,
            report: report.clone(),
            operations: operations.clone(),
        });
        let running = Running { task: Arc::new(task), settlement, report };
        let entry = Entry {
            snapshot: Snapshot {
                instance,
                kind: registration.definition.kind(),
                leases: 1,
                lifecycle: lifecycle.status(),
                health: (registration.definition.kind() == Kind::Service).then_some(ServiceStatus::Starting),
                failure: None,
            },
            internal: registration.internal,
            leases: BTreeSet::from([demand]),
            running: running.clone(),
            stopping: false,
        };
        let lease = Lease::new(running, demand, registration.generation);
        self.track(instance, &lifecycle, holder);
        let retired = state.instances.insert(key, entry);
        self.shared.publish(&state);
        drop(state);
        drop(retired);
        Ok(Acquisition::Lease(lease))
    }

    fn track(&self, id: OperationInstanceId, lifecycle: &Arc<Instance>, holder: &Identity) {
        if let Some(index) = self.shared.index.upgrade() {
            index.track_resident(id, lifecycle, holder.clone());
        }
    }

    // Start a definition that can run once, owned by the registry until the
    // runtime shuts down.
    pub(crate) fn start_once(
        &self, operations: &super::Manager, definition: Definition, runner: Arc<dyn Runner>, internal: bool, reserved: bool,
    ) -> Result<Lease> {
        let id = definition.id().to_owned();
        let resources = runner.resources();
        self.register(definition, runner, Some(&self.root()), internal, reserved)?;
        let root = self.root();
        match self.acquire(operations, &id, &root, resources, None, internal, &Identity::Host)? {
            Acquisition::Lease(lease) => Ok(lease),
            Acquisition::Wait(..) => {
                unreachable!("a newly registered resident definition has no instance")
            }
        }
    }

    pub(crate) fn hold(&self, lease: Lease) {
        self.shared.assembly.lock().expect("resident assembly is healthy").push(lease);
    }

    pub(crate) fn status(&self, key: &Key) -> Option<Snapshot> {
        self.shared.statuses.borrow().get(key).cloned()
    }

    pub(crate) fn statuses(&self) -> BTreeMap<Key, Snapshot> {
        self.shared.statuses.borrow().clone()
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<BTreeMap<Key, Snapshot>> {
        self.shared.statuses.subscribe()
    }

    // Stop every instance over a scope, settle them and forget the scope with
    // the definitions bound to it. The first outcome is retained.
    #[tracing::instrument(target = "harmos::operations", name = "operations.retire_scope", level = "debug", skip_all)]
    pub(crate) async fn retire_scope(&self, scope: &Arc<Scope>) -> Result<Retirement> {
        self.check_scope(scope)?;
        if let Some(outcome) = scope.retirement.lock().expect("resident scope is healthy").clone() {
            return Ok(Retirement { outcome, _scope: Arc::clone(scope), _running: Vec::new(), _definitions: Vec::new() });
        }
        let (running, definitions) = {
            let mut state = self.state();
            scope.closed.store(true, Ordering::Release);
            let definitions: Vec<_> = state.definitions.values().filter(|entry| entry.scope == Some(scope.id)).cloned().collect();
            for definition in &definitions {
                definition.closing.store(true, Ordering::Release);
            }
            let running = state.stop(|key, _, _| key.scope == scope.id);
            self.shared.publish(&state);
            (running, definitions)
        };
        let outcome = settle(running.clone()).await.map_err(Arc::new);
        let (original, detached) = {
            let mut state = self.state();
            if state.instances.iter().any(|(key, entry)| key.scope == scope.id && !entry.running.settlement.lifecycle().is_settled()) {
                return Err(match &outcome {
                    Err(source) => Error::TaskFailed { domain: "resident cleanup", source: source.clone() },
                    Ok(()) => Error::NotReady { domain: "resident cleanup" },
                });
            }
            // Only observer retirement failures resurface at shutdown, as before.
            if let Err(source) = &outcome
                && running.iter().any(|running| running.report.kind == Kind::Observer)
            {
                state.retired_failure.get_or_insert_with(|| Arc::clone(source));
            }
            let original = state.scopes.remove(&scope.id).unwrap_or_else(|| Arc::clone(scope));
            state.addresses.remove(&original.address);
            state.instances.retain(|key, _| key.scope != scope.id);
            for definition in &definitions {
                if state.definitions.get(definition.definition.id()).is_some_and(|current| Arc::ptr_eq(current, definition)) {
                    state.definitions.remove(definition.definition.id());
                }
            }
            self.shared.publish(&state);
            let detached = std::mem::take(&mut *original.detached.lock().expect("resident scope is healthy"));
            (original, detached)
        };
        drop(detached);
        let outcome = scope.retirement.lock().expect("resident scope is healthy").get_or_insert(outcome).clone();
        Ok(Retirement { outcome, _scope: original, _running: running, _definitions: definitions })
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.unregister", level = "debug", skip_all)]
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) fn close_plugin(&self, scope: &crate::plugin::PluginScope) {
        let mut state = self.state();
        let ids: std::collections::BTreeSet<_> = state
            .definitions
            .iter()
            .filter(|(_, entry)| entry.plugin.as_ref().is_some_and(|owner| owner.same_activation(scope)))
            .map(|(id, entry)| {
                entry.closing.store(true, Ordering::Release);
                id.clone()
            })
            .collect();
        state.stop(|key, _, _| ids.contains(&key.definition));
        self.shared.publish(&state);
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(crate) async fn shutdown_plugin(&self, scope: &crate::plugin::PluginScope) -> Result<()> {
        let ids: Vec<_> = self
            .state()
            .definitions
            .iter()
            .filter(|(_, entry)| entry.plugin.as_ref().is_some_and(|owner| owner.same_activation(scope)))
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.unregister(&id).await?;
        }
        Ok(())
    }

    pub(crate) async fn unregister(&self, id: &str) -> Result<()> {
        let (registration, running) = {
            let mut state = self.state();
            let registration =
                state.definitions.get(id).cloned().ok_or_else(|| Error::NotRegistered { domain: "resident", name: id.to_owned() })?;
            registration.closing.store(true, Ordering::Release);
            let running = state.stop(|key, _, _| key.definition == id);
            self.shared.publish(&state);
            (registration, running)
        };
        let result = settle(running.clone()).await;
        if running.iter().any(|running| !running.settlement.lifecycle().is_settled()) {
            return result;
        }
        let mut state = self.state();
        let mut retired = Vec::new();
        if state.definitions.get(id).is_some_and(|current| Arc::ptr_eq(current, &registration)) {
            state.definitions.remove(id);
            let keys: Vec<_> = state.instances.keys().filter(|key| key.definition == id).cloned().collect();
            retired.extend(keys.into_iter().filter_map(|key| state.instances.remove(&key)));
            self.shared.publish(&state);
        }
        drop(state);
        drop((registration, retired));
        result
    }

    pub(crate) fn cleanup_pending(&self, selected: impl Fn(Kind, bool) -> bool) -> bool {
        self.state()
            .instances
            .values()
            .any(|entry| selected(entry.snapshot.kind, entry.internal) && !entry.running.settlement.lifecycle().is_settled())
    }

    // Refuse new activations and stop observers and application services.
    // Infrastructure services keep running until `shutdown`.
    pub(crate) fn close_admission(&self) {
        let scopes = {
            let mut state = self.state();
            state.closed = true;
            for scope in state.scopes.values() {
                scope.closed.store(true, Ordering::Release);
            }
            state.stop(|_, _, internal| !internal);
            self.shared.publish(&state);
            state.release_scopes()
        };
        // Releasing the final resource reference can invoke author Drop code.
        drop(scopes);
    }

    // Stop every instance of one kind and settle them; a failed cleanup stays
    // retained and the next stop retries it.
    #[tracing::instrument(target = "harmos::operations", name = "operations.stop_kind", level = "debug", skip_all)]
    pub(crate) async fn stop_kind(&self, kind: Kind) -> Result<()> {
        let running = {
            let mut state = self.state();
            let running = state.stop(|_, selected, _| selected == kind);
            self.shared.publish(&state);
            running
        };
        settle(running).await
    }

    pub(crate) async fn shutdown_observers(&self) -> Result<()> {
        self.close_admission();
        let running = self.select(|kind, _| kind == Kind::Observer);
        let result = settle(running).await;
        let (definitions, scopes, retired_failure) = {
            let mut state = self.state();
            let ids: Vec<_> = state
                .definitions
                .iter()
                .filter(|(id, registration)| {
                    registration.definition.kind() == Kind::Observer
                        && state
                            .instances
                            .iter()
                            .filter(|(key, _)| key.definition == **id)
                            .all(|(_, entry)| entry.running.settlement.lifecycle().is_settled())
                })
                .map(|(id, _)| id.clone())
                .collect();
            let definitions: Vec<_> = ids.into_iter().filter_map(|id| state.definitions.remove(&id)).collect();
            (definitions, state.release_scopes(), state.retired_failure.clone())
        };
        // Context factories may retain runtime handles. Drop them outside
        // the registry lock after every active use has joined.
        drop((definitions, scopes));
        result?;
        match retired_failure {
            Some(source) => Err(Error::TaskFailed { domain: "retired observer", source }),
            None => Ok(()),
        }
    }

    pub(crate) async fn shutdown_application(&self) -> Result<()> {
        let running = {
            let mut state = self.state();
            state.closed = true;
            let running = state.stop(|_, _, internal| !internal);
            self.shared.publish(&state);
            running
        };
        settle(running).await
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.shutdown", level = "debug", skip_all)]
    pub(crate) async fn shutdown(&self) -> Result<()> {
        let running = {
            let mut state = self.state();
            state.closed = true;
            let running = state.stop(|_, _, _| true);
            self.shared.publish(&state);
            running
        };
        let retained = running.clone();
        let result = settle(running).await;
        if retained.iter().any(|running| !running.settlement.lifecycle().is_settled()) {
            return result;
        }
        let (definitions, scopes) = {
            let mut state = self.state();
            for scope in state.scopes.values() {
                scope.closed.store(true, Ordering::Release);
            }
            state.addresses.clear();
            (std::mem::take(&mut state.definitions), std::mem::take(&mut state.scopes))
        };
        let assembly = std::mem::take(&mut *self.shared.assembly.lock().expect("resident assembly is healthy"));
        let detached: Vec<_> =
            scopes.values().flat_map(|scope| std::mem::take(&mut *scope.detached.lock().expect("resident scope is healthy"))).collect();
        drop((definitions, scopes, assembly, detached));
        result
    }

    fn select(&self, selected: impl Fn(Kind, bool) -> bool) -> Vec<Running> {
        self.state()
            .instances
            .values()
            .filter(|entry| selected(entry.snapshot.kind, entry.internal))
            .map(|entry| entry.running.clone())
            .collect()
    }

    fn check_scope(&self, scope: &Scope) -> Result<()> {
        if !Weak::ptr_eq(&scope.owner, &Arc::downgrade(&self.shared.identity)) {
            return Err(Error::InvalidInput { field: "resident scope", reason: "scope belongs to another manager" });
        }
        Ok(())
    }
}

impl Shared {
    fn publish(&self, state: &State) {
        self.statuses.send_replace(
            state
                .instances
                .iter()
                .map(|(key, entry)| {
                    let mut snapshot = entry.snapshot.clone();
                    snapshot.lifecycle = entry.running.settlement.lifecycle().status();
                    (key.clone(), snapshot)
                })
                .collect(),
        );
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("resident registry is healthy")
    }

    fn release(&self, key: &Key, id: OperationInstanceId, demand: u64) -> bool {
        let mut state = self.state();
        let Some(entry) = state.instances.get_mut(key).filter(|entry| entry.snapshot.instance == id) else {
            return true;
        };
        entry.leases.remove(&demand);
        entry.snapshot.leases = entry.leases.len();
        let final_release = entry.leases.is_empty();
        if final_release {
            entry.stop();
        }
        self.publish(&state);
        final_release
    }

    fn stop_instance(&self, key: &Key, id: OperationInstanceId) -> bool {
        let mut state = self.state();
        let Some(entry) = state.instances.get_mut(key).filter(|entry| entry.snapshot.instance == id) else {
            return false;
        };
        entry.leases.clear();
        entry.snapshot.leases = 0;
        entry.stop();
        self.publish(&state);
        true
    }
}

impl State {
    fn admit(&self) -> Result<()> {
        if self.closed { Err(Error::Closing { domain: "resident" }) } else { Ok(()) }
    }

    fn next(&mut self) -> Result<u64> {
        self.next =
            self.next.checked_add(1).ok_or(Error::InvalidInput { field: "resident identity", reason: "identity space exhausted" })?;
        Ok(self.next)
    }

    fn stop(&mut self, selected: impl Fn(&Key, Kind, bool) -> bool) -> Vec<Running> {
        self.instances
            .iter_mut()
            .filter(|(key, entry)| selected(key, entry.snapshot.kind, entry.internal))
            .map(|(_, entry)| {
                entry.leases.clear();
                entry.snapshot.leases = 0;
                entry.stop();
                entry.running.clone()
            })
            .collect()
    }

    // Forget scopes whose instances all settled; the root scope stays.
    fn release_scopes(&mut self) -> Vec<Arc<Scope>> {
        let ids: Vec<_> = self
            .scopes
            .keys()
            .filter(|id| {
                **id != ROOT
                    && self
                        .instances
                        .iter()
                        .filter(|(key, _)| key.scope == **id)
                        .all(|(_, entry)| entry.running.settlement.lifecycle().is_settled())
            })
            .copied()
            .collect();
        let scopes: Vec<_> = ids.into_iter().filter_map(|id| self.scopes.remove(&id)).collect();
        for scope in &scopes {
            self.addresses.remove(&scope.address);
        }
        scopes
    }
}

impl Entry {
    fn stop(&mut self) {
        self.stopping = true;
        if let Some(health) = &mut self.snapshot.health
            && self.running.settlement.lifecycle().status().outcome().is_none()
        {
            *health = ServiceStatus::Stopping;
        }
        self.running.stop();
    }
}

impl Running {
    fn stop(&self) {
        self.settlement.lifecycle().cancel();
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.settle", level = "debug", skip_all)]
    async fn settle(&self) -> std::result::Result<Arc<Outcome>, Arc<tokio::task::JoinError>> {
        if let Err(failure) = self.settlement.lifecycle().settle_dependents().await {
            return Ok(Arc::new(Err(failure)));
        }
        let settlement = Arc::clone(&self.settlement);
        let report = self.report.clone();
        self.task.settle_retry(move || settlement.retry(report)).await
    }

    pub(crate) fn lifecycle(&self) -> &Arc<Instance> {
        self.settlement.lifecycle()
    }
}

impl Report {
    pub(crate) fn id(&self) -> OperationInstanceId {
        self.id
    }

    pub(crate) fn key(&self) -> &Key {
        &self.key
    }

    fn with_entry(&self, update: impl FnOnce(&mut Entry) -> bool) {
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut state = owner.state();
        let changed = state.instances.get_mut(&self.key).filter(|entry| entry.snapshot.instance == self.id).map(update).unwrap_or(false);
        if changed {
            owner.publish(&state);
        }
    }

    // Move the instance's lifecycle to `phase` unless it is already stopping.
    pub(crate) fn transition(&self, phase: Phase) {
        self.with_entry(|entry| {
            if entry.stopping && !matches!(phase, Phase::Stopping | Phase::Finalizing) {
                return false;
            }
            entry.running.lifecycle().transition(phase);
            true
        });
    }

    // Close lease admission: a stopping instance is never joined again.
    pub(crate) fn quiesce(&self) {
        self.with_entry(|entry| {
            entry.stopping = true;
            if let Some(health) = &mut entry.snapshot.health {
                *health = ServiceStatus::Stopping;
            }
            true
        });
    }

    // Publish a service's health; its lifecycle follows.
    pub(crate) fn health(&self, status: ServiceStatus) {
        self.with_entry(|entry| {
            if entry.stopping && !matches!(status, ServiceStatus::Stopped | ServiceStatus::Inactive { .. }) {
                return false;
            }
            let lifecycle = entry.running.lifecycle();
            match &status {
                ServiceStatus::Running { .. } => lifecycle.transition(Phase::Running),
                ServiceStatus::BackingOff => lifecycle.transition(Phase::Restarting),
                _ => {}
            }
            entry.snapshot.health = Some(status);
            true
        });
    }

    pub(crate) fn publish(&self) {
        self.with_entry(|_| true);
    }

    // Record the instance's outcome as its task or a cleanup retry completes.
    pub(crate) fn complete(&self, outcome: &Outcome) {
        self.with_entry(|entry| {
            entry.running.settlement.record(outcome);
            entry.snapshot.failure = outcome.as_ref().err().cloned();
            if let Some(health) = &mut entry.snapshot.health {
                if outcome.is_err() && !matches!(health, ServiceStatus::Inactive { .. }) {
                    *health = ServiceStatus::Inactive { cause: crate::services::ServiceExit::Panicked, history: Vec::new() };
                } else if outcome.is_ok() {
                    *health = ServiceStatus::Stopped;
                }
            }
            true
        });
    }
}

impl Lease {
    fn new(running: Running, demand: u64, generation: OperationGeneration) -> Self {
        Self { running, demand, generation, released: false, joins_release: false }
    }

    pub(crate) fn id(&self) -> OperationInstanceId {
        self.running.report.id
    }

    pub(crate) fn generation(&self) -> OperationGeneration {
        self.generation
    }

    pub(crate) fn lifecycle(&self) -> Arc<Instance> {
        Arc::clone(self.running.lifecycle())
    }

    // Settle this instance; holding the lease keeps it running.
    pub(crate) fn settlement(&self) -> impl Future<Output = Result<()>> + Send + 'static + use<> {
        let running = self.running.clone();
        async move { settle(vec![running]).await }
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn transition(&self, phase: Phase) {
        self.running.report.transition(phase);
    }

    // Stop the instance for every lease holder.
    pub(crate) fn stop(&self) {
        let report = &self.running.report;
        let stopped = report.owner.upgrade().is_some_and(|owner| owner.stop_instance(&report.key, report.id));
        if !stopped {
            self.running.stop();
        }
    }

    // Hold the lease until its scope retires or the runtime shuts down.
    pub(crate) fn detach(self) {
        let report = &self.running.report;
        let Some(owner) = report.owner.upgrade() else {
            return;
        };
        let scope = owner.state().scopes.get(&report.key.scope).cloned();
        match scope {
            Some(scope) => scope.detached.lock().expect("resident scope is healthy").push(self),
            None => drop(self),
        }
    }

    // Release this lease and, when it was an infrastructure definition's last,
    // forget the definition once its instance settled.
    pub(crate) fn retire(&mut self) -> impl Future<Output = Result<()>> + Send + 'static + use<> {
        let completion = self.begin_release();
        let report = self.running.report.clone();
        let lifecycle = Arc::clone(self.running.lifecycle());
        async move {
            let result = completion.await;
            if !lifecycle.is_settled() {
                return result;
            }
            if let Some(owner) = report.owner.upgrade() {
                let mut state = owner.state();
                let owns = state.instances.get(&report.key).is_some_and(|entry| entry.snapshot.instance == report.id && entry.internal);
                let retired = owns.then(|| (state.instances.remove(&report.key), state.definitions.remove(&report.key.definition)));
                owner.publish(&state);
                drop(state);
                drop(retired);
            }
            result
        }
    }

    pub(crate) async fn release(mut self) -> Result<()> {
        self.begin_release().await
    }

    fn begin_release(&mut self) -> impl Future<Output = Result<()>> + Send + 'static + use<> {
        self.request();
        let running = self.joins_release.then(|| self.running.clone());
        async move { settle(running.into_iter().collect()).await }
    }

    fn request(&mut self) -> bool {
        if self.released {
            return self.joins_release;
        }
        self.released = true;
        let report = &self.running.report;
        self.joins_release = match report.owner.upgrade() {
            Some(owner) => owner.release(&report.key, report.id, self.demand),
            None => {
                self.running.stop();
                true
            }
        };
        self.joins_release
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.request();
    }
}

impl Staged {
    pub(crate) fn publication(&self) -> Arc<AtomicBool> {
        self.published.clone()
    }

    pub fn commit(self) -> Result<()> {
        let owner = self.owner.upgrade().ok_or(Error::Closing { domain: "resident" })?;
        let state = owner.state();
        state.admit()?;
        if self.scope.closed.load(Ordering::Acquire) {
            return Err(Error::Closing { domain: "resident scope" });
        }
        for entry in &self.entries {
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            if entry.plugin.as_ref().is_some_and(|scope| scope.is_closed()) {
                return Err(Error::Closing { domain: "plugin scope" });
            }
            if entry.closing.load(Ordering::Acquire)
                || !state.definitions.get(entry.definition.id()).is_some_and(|current| Arc::ptr_eq(current, entry))
            {
                return Err(Error::NotRegistered { domain: "resident reservation", name: entry.definition.id().to_owned() });
            }
        }
        self.published.store(true, Ordering::Release);
        Ok(())
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if self.published.load(Ordering::Acquire) {
            return;
        }
        let Some(owner) = self.owner.upgrade() else {
            return;
        };
        let mut state = owner.state();
        for entry in &self.entries {
            if state.definitions.get(entry.definition.id()).is_some_and(|current| Arc::ptr_eq(current, entry)) {
                state.definitions.remove(entry.definition.id());
            }
        }
        // self.entries retains every runner until this lock has been released.
    }
}

impl Retirement {
    pub(crate) fn into_outcome(self) -> Outcome {
        self.outcome
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let state = self.state.get_mut().expect("resident registry is healthy");
        state.stop(|_, _, _| true);
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[tracing::instrument(target = "harmos::operations", name = "operations.settle", level = "debug", skip_all)]
pub(crate) async fn settle(running: Vec<Running>) -> Result<()> {
    let pending = running.into_iter().map(|running| async move {
        let domain = running.report.kind.domain();
        match running.settle().await {
            Ok(outcome) => outcome.as_ref().clone().map_err(|source| Error::TaskFailed { domain, source }),
            Err(source) => Err(Error::TaskFailed {
                domain: match running.report.kind {
                    Kind::Observer => "observer supervisor",
                    Kind::Service => "service supervisor",
                },
                source,
            }),
        }
    });
    supervision::settle_all(pending).await
}

fn address(resources: &Resources) -> usize {
    Arc::as_ptr(resources) as *const () as usize
}

fn validate(definition: &Definition) -> Result<()> {
    match definition {
        Definition::Observer(definition) => crate::observers::validate(definition),
        Definition::Service(definition) => crate::services::validate(definition),
    }
}

/* === Tests === */
