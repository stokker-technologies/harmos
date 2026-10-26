/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Own Runtime Plugin Registration, Activation And Retained Cleanup.

/* === Modules === */

#[cfg(feature = "plugin-guest")]
mod guest;
mod progress;
#[cfg(feature = "plugin-sidecar")]
mod sidecar;

/* === Re-Exports === */

pub(crate) use progress::Driving;
#[cfg(feature = "plugin-sidecar")]
pub(crate) use progress::Lease;

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

// Third Party Imports
use tokio::sync::{mpsc, oneshot};

// Local Imports
use super::{
    PluginArtifact, PluginBackend, PluginCleanup, PluginDefinition, PluginDefinitions, PluginLifecycle, PluginLifecycleAuthority,
    PluginManager, PluginStatus, PluginTransition, Router,
};
use crate::operations::{Bindings, Contract, Lane, Manager, OperationDefinition, PluginRegistration};
use crate::supervision::Task;
use crate::{Application, Error, Result};

/* === Constants === */

const COMMANDS: usize = 64;

/* === Macros === */

/* === Type Aliases === */

pub(super) type LifecycleFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
pub(super) type Driver = Box<dyn Execution>;
pub(super) type Factory = Arc<dyn Fn() -> Result<Driver> + Send + Sync>;
type Backends = PluginBackend<Driver, Driver, Driver>;
type Registry = PluginManager<Driver, Driver, Driver>;
type Bind = Arc<dyn Fn(&mut Bindings<'_>) -> Result<()> + Send + Sync>;
type Snapshots = Arc<RwLock<BTreeMap<String, PluginStatus>>>;

/* === Structs === */

// The Runtime's Shared Plugin Registration And Lifecycle Owner.
pub struct PluginHost<A: Application> {
    shared: Arc<Shared<A>>,
}

struct Shared<A: Application> {
    commands: mpsc::Sender<Command>,
    statuses: Snapshots,
    closing: AtomicBool,
    stop_requests: tokio::sync::watch::Sender<Option<String>>,
    task: Task<std::result::Result<(), Arc<Error>>>,
    operations: Manager,
    cleanup: PluginCleanup,

    #[cfg(feature = "plugin-sidecar")]
    handles: Arc<crate::services::Handles<A>>,

    #[cfg(feature = "plugin-sidecar")]
    driving: Driving,

    application: PhantomData<fn() -> A>,
}

struct Lifecycle {
    driving: Driving,
    operations: Manager,
    sources: BTreeMap<String, Source>,
}

struct Source {
    factory: Factory,
    grants: BTreeSet<crate::Identifier>,
    enabled: bool,
}

struct Progressing {
    driver: Driver,
    driving: Driving,
}

struct Embedded {
    operations: Manager,
    bind: Bind,
    registration: Option<PluginRegistration>,
    endpoint: Option<super::PluginEndpoint>,
}

/* === Enums === */

enum Command {
    Reload {
        id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    Remove {
        id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    Stop {
        id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    Register {
        definition: Box<PluginDefinition>,
        grants: BTreeSet<crate::Identifier>,
        factory: Factory,
        restart: crate::config::PluginRestartSettings,
        replace: bool,
        reply: oneshot::Sender<Result<()>>,
    },
    Initialize {
        reply: oneshot::Sender<Result<()>>,
    },
}

enum Event {
    Command(Option<Command>),
    Closing,
    Exited(String, Result<()>),
    Retry(String),
}

enum Initialization {
    Complete(Result<()>),
    Cancelled,
    Closing,
}

/* === Traits === */

pub(super) trait Execution: Send + 'static {
    fn start<'a>(
        &'a mut self, definition: &'a PluginDefinition, router: &'a Router, authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a>;
    fn stop<'a>(&'a mut self, definition: &'a PluginDefinition, authority: &'a PluginLifecycleAuthority<'a>) -> LifecycleFuture<'a>;
    fn poll(&mut self, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Pending
    }
}

/* === Implementations === */

impl<A: Application> Clone for PluginHost<A> {
    fn clone(&self) -> Self {
        Self { shared: self.shared.clone() }
    }
}

impl<A: Application> PluginHost<A> {
    pub(crate) fn new(operations: Manager, #[cfg(feature = "plugin-sidecar")] handles: crate::services::Handles<A>) -> Result<Self> {
        let mut registry = Registry::new(PluginDefinitions::new([], [])?)?;
        let cleanup = registry.cleanup();
        let statuses: Snapshots = Arc::new(RwLock::new(BTreeMap::new()));
        let snapshots = statuses.clone();
        registry.observe(Arc::new(move |id, status| {
            let mut snapshots = snapshots.write().expect("plugin snapshot owner is healthy");
            if let Some(status) = status {
                snapshots.insert(id.to_owned(), status);
            } else {
                snapshots.remove(id);
            }
        }));
        let (commands, incoming) = mpsc::channel(COMMANDS);
        let driving = Driving::default();
        let lifecycle = Lifecycle { driving: driving.clone(), operations: operations.clone(), sources: BTreeMap::new() };
        let snapshots = statuses.clone();
        let stop_requests = tokio::sync::watch::channel(None).0;
        let interrupts = stop_requests.subscribe();
        let task = Task::spawn(move |stopping| async move {
            run(registry, lifecycle, incoming, stopping, snapshots, interrupts).await.map_err(Arc::new)
        });
        Ok(Self {
            shared: Arc::new(Shared {
                commands,
                statuses,
                closing: AtomicBool::new(false),
                stop_requests,
                task,
                cleanup,
                operations,
                #[cfg(feature = "plugin-sidecar")]
                handles: Arc::new(handles),
                #[cfg(feature = "plugin-sidecar")]
                driving,
                application: PhantomData,
            }),
        })
    }

    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }

    pub fn status(&self, id: &str) -> Option<PluginStatus> {
        self.shared.statuses.read().expect("plugin snapshot owner is healthy").get(id).cloned()
    }

    pub fn statuses(&self) -> Vec<PluginStatus> {
        self.shared.statuses.read().expect("plugin snapshot owner is healthy").values().cloned().collect()
    }

    pub fn cleanup(&self) -> PluginCleanup {
        self.shared.cleanup.clone()
    }

    pub async fn register_embedded<L: Lane>(
        &self, definition: PluginDefinition, restart: crate::config::PluginRestartSettings,
        registration: crate::operations::Registration<L>,
    ) -> Result<()> {
        self.embedded(definition, restart, registration, false).await
    }

    pub async fn reload_embedded<L: Lane>(
        &self, definition: PluginDefinition, restart: crate::config::PluginRestartSettings,
        registration: crate::operations::Registration<L>,
    ) -> Result<()> {
        self.embedded(definition, restart, registration, true).await
    }

    async fn embedded<L: Lane>(
        &self, definition: PluginDefinition, restart: crate::config::PluginRestartSettings,
        registration: crate::operations::Registration<L>, replace: bool,
    ) -> Result<()> {
        let declarations = registration.batch.operations.iter().map(|factory| factory.definition.clone()).collect();
        let contracts = registration
            .batch
            .operations
            .iter()
            .map(|factory| factory.contract())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|source| Error::OperationFailed { operation: definition.id.clone(), source: Arc::new(source) })?;
        let bind: Bind = Arc::new(move |bindings| bindings.register(registration.clone()));
        self.embedded_catalog(definition, declarations, contracts, bind, restart, replace).await
    }

    async fn embedded_catalog(
        &self, mut definition: PluginDefinition, declarations: Vec<OperationDefinition>, mut contracts: Vec<Contract>, bind: Bind,
        restart: crate::config::PluginRestartSettings, replace: bool,
    ) -> Result<()> {
        if !matches!(definition.artifact, PluginArtifact::Embedded { .. }) || definition.contributions.lifecycle.is_some() {
            return Err(Error::InvalidInput {
                field: "embedded plugin",
                reason: "operation catalogs require an embedded artifact without a portable SDK lifecycle",
            });
        }
        if !definition.contributions.operations.is_empty() {
            harmos_domain::inventory::operations::validate_match(&declarations, &definition.contributions.operations).map_err(|_| {
                Error::InvalidInput { field: "embedded plugin catalog", reason: "the catalog differs from the supplied definition" }
            })?;
        }
        contracts.sort_by(|a, b| a.operation().id.cmp(&b.operation().id));
        if !definition.contributions.wire_contracts.is_empty() {
            let supplied = &mut definition.contributions.wire_contracts;
            supplied.sort_by(|a, b| a.operation().id.cmp(&b.operation().id));
            if *supplied != contracts {
                return Err(Error::InvalidInput {
                    field: "embedded plugin contracts",
                    reason: "wire contracts differ from the authored handler catalog",
                });
            }
        }
        definition.contributions.operations = declarations;
        definition.contributions.wire_contracts = contracts;
        let operations = self.shared.operations.clone();
        let factory: Factory = Arc::new(move || {
            Ok(Box::new(Embedded { operations: operations.clone(), bind: bind.clone(), registration: None, endpoint: None }))
        });
        self.register(definition, factory, BTreeSet::new(), restart, replace).await
    }

    pub(super) async fn register(
        &self, definition: PluginDefinition, factory: Factory, grants: BTreeSet<crate::Identifier>,
        restart: crate::config::PluginRestartSettings, replace: bool,
    ) -> Result<()> {
        self.admit()?;
        restart.validate()?;
        let (reply, outcome) = oneshot::channel();
        self.shared
            .commands
            .send(Command::Register { definition: Box::new(definition), grants, factory, restart, replace, reply })
            .await
            .map_err(|_| closing())?;
        outcome.await.map_err(|_| closing())?
    }

    // Restart This Frozen Definition And Its Active Dependents.
    pub async fn reload(&self, id: &str) -> Result<()> {
        self.admit()?;
        let (reply, outcome) = oneshot::channel();
        self.shared.commands.send(Command::Reload { id: id.to_owned(), reply }).await.map_err(|_| closing())?;
        outcome.await.map_err(|_| closing())?
    }

    // Retire And Unregister A Plugin With No Registered Dependents.
    pub async fn remove(&self, id: &str) -> Result<()> {
        self.admit()?;
        let (reply, outcome) = oneshot::channel();
        self.shared.commands.send(Command::Remove { id: id.to_owned(), reply }).await.map_err(|_| closing())?;
        outcome.await.map_err(|_| closing())?
    }

    pub async fn stop(&self, id: &str) -> Result<()> {
        self.admit()?;
        if self.status(id).is_none() {
            return Err(Error::NotRegistered { domain: "plugin", name: id.to_owned() });
        }
        self.shared.stop_requests.send_replace(Some(id.to_owned()));
        let (reply, outcome) = oneshot::channel();
        self.shared.commands.send(Command::Stop { id: id.to_owned(), reply }).await.map_err(|_| closing())?;
        outcome.await.map_err(|_| closing())?
    }

    pub async fn initialize(&self) -> Result<()> {
        self.admit()?;
        let (reply, outcome) = oneshot::channel();
        self.shared.commands.send(Command::Initialize { reply }).await.map_err(|_| closing())?;
        outcome.await.map_err(|_| closing())?
    }

    pub(crate) fn close_admission(&self) {
        self.shared.closing.store(true, Ordering::Release);
        self.shared.task.stop();
    }

    pub async fn shutdown(&self) -> Result<()> {
        let retry = self.shared.closing.swap(true, Ordering::AcqRel);
        self.shared.task.stop();
        let settled = self.shared.task.settle().await.map_err(|source| Error::TaskFailed { domain: "plugin owner", source })?;
        match settled.as_ref() {
            Ok(()) => Ok(()),
            Err(_) if retry => self.shared.cleanup.retry().await,
            Err(source) => Err(Error::TaskFailed { domain: "plugin shutdown", source: source.clone() }),
        }
    }

    fn admit(&self) -> Result<()> {
        if self.shared.closing.load(Ordering::Acquire) {
            return Err(closing());
        }
        Ok(())
    }
}

impl PluginLifecycle for Lifecycle {
    type Guest = Driver;
    type Sidecar = Driver;
    type Embedded = Driver;

    fn preflight(&mut self, candidate: &PluginDefinitions) -> Result<()> {
        for (consumer, required) in candidate.host_requirements() {
            let source =
                self.sources.get(&consumer.id).ok_or(Error::NotRegistered { domain: "plugin backend", name: consumer.id.clone() })?;
            if !source.grants.contains(&required.contract.operation().id) {
                return Err(Error::InvalidInput {
                    field: "plugin host permission",
                    reason: "every host-routed operation requires an explicit grant",
                });
            }
        }
        self.operations.validate_requirements(candidate)
    }

    fn prepare(&mut self, definition: &PluginDefinition) -> Result<Backends> {
        let source =
            self.sources.get(&definition.id).ok_or(Error::NotRegistered { domain: "plugin backend", name: definition.id.clone() })?;
        let driver: Driver = Box::new(Progressing { driver: (source.factory)()?, driving: self.driving.clone() });
        Ok(match definition.kind() {
            super::PluginKind::Guest => PluginBackend::Guest(driver),
            super::PluginKind::Sidecar => PluginBackend::Sidecar(driver),
            super::PluginKind::Embedded => PluginBackend::Embedded(driver),
        })
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends,
    ) -> Result<()> {
        driver(backend).start(definition, router, authority).await
    }

    async fn stop(definition: &PluginDefinition, authority: &PluginLifecycleAuthority<'_>, backend: &mut Backends) -> Result<()> {
        driver(backend).stop(definition, authority).await
    }
}

impl Execution for Progressing {
    fn start<'a>(
        &'a mut self, definition: &'a PluginDefinition, router: &'a Router, authority: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        let driving = self.driving.clone();
        let mut starting = self.driver.start(definition, router, authority);
        Box::pin(std::future::poll_fn(move |context| {
            driving.tick(context);
            starting.as_mut().poll(context)
        }))
    }

    fn stop<'a>(&'a mut self, definition: &'a PluginDefinition, authority: &'a PluginLifecycleAuthority<'a>) -> LifecycleFuture<'a> {
        let driving = self.driving.clone();
        let mut stopping = self.driver.stop(definition, authority);
        Box::pin(std::future::poll_fn(move |context| {
            driving.tick(context);
            stopping.as_mut().poll(context)
        }))
    }

    fn poll(&mut self, context: &mut Context<'_>) -> Poll<Result<()>> {
        self.driver.poll(context)
    }
}

impl Execution for Embedded {
    fn start<'a>(
        &'a mut self, definition: &'a PluginDefinition, router: &'a Router, _: &'a PluginLifecycleAuthority<'a>,
    ) -> LifecycleFuture<'a> {
        Box::pin(async move {
            self.endpoint = Some(router.enter(&definition.id).map_err(super::PluginFailure::Routing)?);
            let scope = self.endpoint.as_ref().unwrap().scope().unwrap();
            self.registration = Some(self.operations.stage_plugin(scope, |bindings| (self.bind)(bindings))?);
            Ok(())
        })
    }

    fn stop<'a>(&'a mut self, _: &'a PluginDefinition, _: &'a PluginLifecycleAuthority<'a>) -> LifecycleFuture<'a> {
        Box::pin(async move {
            if let Some(registration) = &mut self.registration {
                registration.shutdown().await?;
            }
            self.registration.take();
            self.endpoint.take();
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn affected_dependents(registry: &Registry, id: &str) -> Result<BTreeSet<String>> {
    let mut selected = BTreeSet::from([id.to_owned()]);
    let mut pending = vec![id.to_owned()];
    while let Some(owner) = pending.pop() {
        for dependent in registry.dependents(&owner)? {
            if selected.insert(dependent.clone()) {
                pending.push(dependent);
            }
        }
    }
    Ok(selected)
}

fn recovering(registry: &Registry, lifecycle: &Lifecycle, id: &str) -> Result<BTreeSet<String>> {
    let mut selected = affected_dependents(registry, id)?;
    selected.retain(|owner| lifecycle.sources.get(owner).is_some_and(|source| source.enabled));
    loop {
        let mut blocked = Vec::new();
        for owner in &selected {
            if registry.dependencies(owner)?.iter().any(|provider| {
                !selected.contains(provider) && !registry.status(provider).is_some_and(|status| status.state == super::PluginState::Ready)
            }) {
                blocked.push(owner.clone());
            }
        }
        if blocked.is_empty() {
            break;
        }
        for owner in blocked {
            selected.remove(&owner);
        }
    }
    Ok(selected)
}

fn schedule_failure(registry: &mut Registry, lifecycle: &Lifecycle, outcome: &Result<()>) {
    if let Err(Error::PluginLifecycle { source }) = outcome
        && let super::PluginFailure::Start { artifact, cleanup, .. } = source.as_ref()
        && cleanup.is_empty()
        && lifecycle.sources.get(artifact).is_some_and(|source| source.enabled)
    {
        registry.schedule_retry(artifact);
    }
}

async fn run(
    mut registry: Registry, mut lifecycle: Lifecycle, mut incoming: mpsc::Receiver<Command>, stopping: tokio::sync::watch::Sender<bool>,
    snapshots: Snapshots, mut interrupts: tokio::sync::watch::Receiver<Option<String>>,
) -> Result<()> {
    let mut stopped = stopping.subscribe();
    loop {
        if *stopped.borrow() {
            break;
        }
        let retry = registry.scheduled_retry();
        let retrying = async {
            let Some((id, at)) = retry else {
                return std::future::pending().await;
            };
            crate::supervision::executor::sleep(at.saturating_duration_since(crate::supervision::executor::now())).await;
            id
        };
        let event = tokio::select! {
            biased;
            _ = stopped.changed() => Event::Closing,
            command = incoming.recv() => Event::Command(command),
            id = retrying => Event::Retry(id),
            (id, outcome) = std::future::poll_fn(|context| registry.poll_backends(context, |backend, context| driver(backend).poll(context))) => Event::Exited(id, outcome),
        };
        let command = match event {
            Event::Command(Some(command)) => command,
            Event::Command(None) | Event::Closing => break,
            Event::Retry(id) => {
                registry.cancel_retry(&id);
                if interrupts.has_changed().unwrap_or(false) || !lifecycle.sources.get(&id).is_some_and(|source| source.enabled) {
                    publish(&registry, &snapshots);
                    continue;
                }
                let previous = registry.ready_generations();
                let selected = recovering(&registry, &lifecycle, &id)?;
                let outcome = tokio::select! {
                    biased;
                    _ = stopped.changed() => Initialization::Closing,
                    _ = interrupts.changed() => Initialization::Cancelled,
                    result = registry.initialize_selected(&selected, &mut lifecycle) => Initialization::Complete(result),
                };
                match outcome {
                    Initialization::Complete(result) => schedule_failure(&mut registry, &lifecycle, &result),
                    Initialization::Cancelled => {
                        let _ = registry.cancel_initialization(&previous).await;
                    }
                    Initialization::Closing => break,
                }
                publish(&registry, &snapshots);
                continue;
            }
            Event::Exited(id, outcome) => {
                let retired = registry.retire_exit(&id, outcome).await;
                if retired.is_ok() && lifecycle.sources.get(&id).is_some_and(|source| source.enabled) {
                    registry.schedule_retry(&id);
                }
                publish(&registry, &snapshots);
                continue;
            }
        };
        match command {
            Command::Reload { id, mut reply } => {
                if reply.is_closed() {
                    continue;
                }
                let result = async {
                    let mut selected = affected_dependents(&registry, &id)?;
                    selected.retain(|owner| owner == &id || lifecycle.sources.get(owner).is_some_and(|source| source.enabled));
                    registry.activation_cycle(Some(&selected));
                    lifecycle.sources.get_mut(&id).ok_or(Error::NotRegistered { domain: "plugin", name: id.clone() })?.enabled = true;
                    let candidate = PluginDefinitions::new(registry.definitions().cloned(), [id.clone()])?;
                    let previous = registry.ready_generations();
                    let outcome = tokio::select! {
                        biased;
                        _ = stopped.changed() => Initialization::Closing,
                        _ = reply.closed() => Initialization::Cancelled,
                        _ = interrupts.changed() => Initialization::Cancelled,
                        result = registry.reload_selected(candidate, &selected, &mut lifecycle) => Initialization::Complete(result),
                    };
                    match outcome {
                        Initialization::Complete(result) => result,
                        Initialization::Cancelled => {
                            registry.cancel_initialization(&previous).await.and(Err(Error::NotReady { domain: "plugin reload cancelled" }))
                        }
                        Initialization::Closing => Err(closing()),
                    }
                }
                .await;
                schedule_failure(&mut registry, &lifecycle, &result);
                let _ = reply.send(result);
            }
            Command::Remove { id, reply } => {
                if reply.is_closed() {
                    continue;
                }
                let result = async {
                    if !registry.dependents(&id)?.is_empty() {
                        return Err(Error::InvalidInput { field: "plugin removal", reason: "registered dependents must be removed first" });
                    }
                    registry.cancel_retry(&id);
                    let candidate = PluginDefinitions::new(registry.definitions().filter(|definition| definition.id != id).cloned(), [])?;
                    registry.reload_selected(candidate, &BTreeSet::from([id.clone()]), &mut lifecycle).await?;
                    lifecycle.sources.remove(&id);
                    Ok(())
                }
                .await;
                let _ = reply.send(result);
            }
            Command::Stop { id, reply } => {
                let result = async {
                    let selected = affected_dependents(&registry, &id)?;
                    for owner in selected {
                        registry.cancel_retry(&owner);
                        if let Some(source) = lifecycle.sources.get_mut(&owner) {
                            source.enabled = false;
                        }
                    }
                    registry.stop_plugin(&id).await
                }
                .await;
                interrupts.borrow_and_update();
                let _ = reply.send(result);
            }
            Command::Register { definition, grants, factory, restart, replace, mut reply } => {
                if reply.is_closed() {
                    continue;
                }
                let definition = *definition;
                let result = async {
                    let exists = registry.definition(&definition.id).is_some();
                    if exists && !replace {
                        return Err(Error::AlreadyRegistered { domain: "plugin", name: definition.id.clone() });
                    }
                    if replace && !exists {
                        return Err(Error::NotRegistered { domain: "plugin", name: definition.id.clone() });
                    }
                    let definitions = registry
                        .definitions()
                        .filter(|current| !replace || current.id != definition.id)
                        .cloned()
                        .chain([definition.clone()]);
                    let rebuilt = if replace { vec![definition.id.clone()] } else { Vec::new() };
                    let candidate = PluginDefinitions::new(definitions, rebuilt)?;
                    for (consumer, required) in candidate.host_requirements() {
                        let allowed = if consumer.id == definition.id {
                            &grants
                        } else {
                            &lifecycle
                                .sources
                                .get(&consumer.id)
                                .ok_or(Error::NotRegistered { domain: "plugin backend", name: consumer.id.clone() })?
                                .grants
                        };
                        if !allowed.contains(&required.contract.operation().id) {
                            return Err(Error::InvalidInput {
                                field: "plugin host permission",
                                reason: "every host-routed operation requires an explicit grant",
                            });
                        }
                    }
                    lifecycle.operations.validate_requirements(&candidate)?;
                    if !replace {
                        registry.register(candidate)?;
                        registry.restart_policy(&definition.id, restart);
                        lifecycle.sources.insert(definition.id, Source { factory, grants, enabled: false });
                        return Ok(());
                    }
                    let id = definition.id.clone();
                    let mut selected = affected_dependents(&registry, &id)?;
                    selected.retain(|owner| owner == &id || lifecycle.sources.get(owner).is_some_and(|source| source.enabled));
                    let revision = registry.current_revision();
                    let previous = lifecycle.sources.insert(id.clone(), Source { factory, grants, enabled: true });
                    registry.activation_cycle(Some(&selected));
                    let generations = registry.ready_generations();
                    let outcome = tokio::select! {
                        biased;
                        _ = stopped.changed() => Initialization::Closing,
                        _ = reply.closed() => Initialization::Cancelled,
                        _ = interrupts.changed() => Initialization::Cancelled,
                        result = registry.reload_selected(candidate, &selected, &mut lifecycle) => Initialization::Complete(result),
                    };
                    let result = match outcome {
                        Initialization::Complete(result) => result,
                        Initialization::Cancelled => registry
                            .cancel_initialization(&generations)
                            .await
                            .and(Err(Error::NotReady { domain: "plugin replacement cancelled" })),
                        Initialization::Closing => Err(closing()),
                    };
                    if registry.current_revision() == revision || registry.definition(&id) != Some(&definition) {
                        if let Some(previous) = previous {
                            lifecycle.sources.insert(id, previous);
                        }
                    } else {
                        registry.restart_policy(&id, restart);
                    }
                    result
                }
                .await;
                schedule_failure(&mut registry, &lifecycle, &result);
                let _ = reply.send(result);
            }
            Command::Initialize { mut reply } => {
                if reply.is_closed() {
                    continue;
                }
                registry.activation_cycle(None);
                for source in lifecycle.sources.values_mut() {
                    source.enabled = true;
                }
                let previous = registry.ready_generations();
                let outcome = tokio::select! {
                    biased;
                    _ = stopped.changed() => Initialization::Closing,
                    _ = reply.closed() => Initialization::Cancelled,
                    _ = interrupts.changed() => Initialization::Cancelled,
                    result = registry.initialize(&mut lifecycle) => Initialization::Complete(result),
                };
                match outcome {
                    Initialization::Complete(result) => {
                        schedule_failure(&mut registry, &lifecycle, &result);
                        let _ = reply.send(result);
                    }
                    Initialization::Cancelled => {
                        let result = registry.cancel_initialization(&previous).await;
                        let _ = reply.send(result.and(Err(Error::NotReady { domain: "plugin initialization cancelled" })));
                    }
                    Initialization::Closing => break,
                }
            }
        }
        publish(&registry, &snapshots);
    }
    let result = registry.shutdown(&mut lifecycle).await;
    publish(&registry, &snapshots);
    result
}

fn publish(registry: &Registry, snapshots: &Snapshots) {
    *snapshots.write().expect("plugin snapshot owner is healthy") = registry.statuses().map(|status| (status.id.clone(), status)).collect();
}

fn driver(backend: &mut Backends) -> &mut Driver {
    match backend {
        PluginBackend::Guest(driver) | PluginBackend::Sidecar(driver) | PluginBackend::Embedded(driver) => driver,
    }
}

fn closing() -> Error {
    Error::Closing { domain: "plugin owner" }
}

/* === Tests === */
