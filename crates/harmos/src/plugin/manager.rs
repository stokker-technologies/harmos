/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod retirement;

/* === Re-Exports === */
pub use retirement::{PluginCleanup, PluginCleanupStatus};

/* === Imports === */
// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::{broadcast, watch};
// Local Imports
use super::{
    DispatchError, PluginBackend, PluginCaller, PluginDefinition, PluginDefinitions, PluginEvent,
    PluginEventKind, PluginEvents, PluginGeneration, PluginGraph, PluginKind, PluginScope,
    PluginState, PluginStatus, ReloadImpact, Router,
};
use crate::supervision::cleanup::{Cleanup, Finalize, Resource};
use crate::{Error, Result};

/* === Constants === */
const EVENTS: usize = 64;

/* === Macros === */

/* === Type Aliases === */
type CleanupOwner = Arc<Cleanup<Box<dyn Resource>>>;
type Stop<G, S, E> = for<'a> fn(
    &'a PluginDefinition,
    &'a PluginScope,
    &'a mut PluginBackend<G, S, E>,
) -> Finalize<'a>;

/* === Structs === */
pub struct PluginManager<G, S, E> {
    definitions: BTreeMap<String, PluginDefinition>,
    instances: BTreeMap<String, PluginInstance<G, S, E>>,
    graph: PluginGraph,
    router: Router,
    failed: BTreeSet<String>,
    identity: PluginGeneration,
    revision: u64,
    events: broadcast::Sender<PluginEvent>,
    retirement: Option<retirement::Wait>,
    cleanup: PluginCleanup,
    executor: Option<tokio::runtime::Handle>,
    retain: fn(&mut PluginInstance<G, S, E>),
}

pub struct PluginInstance<G, S, E> {
    definition: PluginDefinition,
    generation: PluginGeneration,
    scope: PluginScope,
    state: watch::Sender<PluginState>,
    backend: Option<PluginBackend<G, S, E>>,
    stop: Stop<G, S, E>,
    cleanup: Option<CleanupOwner>,
}

struct Retiring<G, S, E> {
    definition: PluginDefinition,
    scope: PluginScope,
    backend: PluginBackend<G, S, E>,
    stop: Stop<G, S, E>,
}

pub struct PluginLifecycleAuthority<'a> {
    scope: &'a PluginScope,
    phase: PluginLifecyclePhase,
    backend_kind: PluginKind,
}

pub struct PluginReloadPlan {
    manager: PluginGeneration,
    revision: u64,
    candidate: PluginDefinitions,
    impact: ReloadImpact,
}

#[derive(Debug)]
pub struct PluginCleanupFailure {
    pub artifact: String,
    pub error: Error,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginLifecyclePhase {
    Initialize,
    Shutdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginTransition {
    Initialize,
    Reload,
    Shutdown,
}

#[derive(Clone, Copy)]
enum FailureScope {
    Instance,
    Group,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginFailure {
    #[error(transparent)]
    Plan(#[from] super::Error),
    #[error(transparent)]
    Routing(#[from] DispatchError),
    #[error("reload plan revision {expected} is stale; manager revision is {actual}")]
    StalePlan { expected: u64, actual: u64 },
    #[error("the application could not fence affected plugin work: {0}")]
    Fence(#[source] Error),
    #[error("{} plugin cleanup operations failed", .0.len())]
    Cleanup(Vec<PluginCleanupFailure>),
    #[error("plugin `{artifact}` could not initialize: {source}")]
    Start {
        artifact: String,
        #[source]
        source: Error,
        cleanup: Vec<PluginCleanupFailure>,
    },
}

/* === Traits === */
pub trait PluginLifecycle {
    type Guest: Send + 'static;
    type Sidecar: Send + 'static;
    type Embedded: Send + 'static;

    fn preflight(&mut self, candidate: &PluginDefinitions) -> Result<()> {
        if candidate.host_requirements().next().is_some() {
            return Err(Error::InvalidInput {
                field: "host provider",
                reason: "host lifecycle must preflight host-routed requirements against its original operation manager",
            });
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        definition: &PluginDefinition,
    ) -> Result<PluginBackend<Self::Guest, Self::Sidecar, Self::Embedded>>;

    fn fence(
        &mut self,
        transition: PluginTransition,
        affected: &[PluginDefinition],
    ) -> impl Future<Output = Result<()>> + Send;
    fn start(
        &mut self,
        definition: &PluginDefinition,
        router: &Router,
        authority: &PluginLifecycleAuthority<'_>,
        backend: &mut PluginBackend<Self::Guest, Self::Sidecar, Self::Embedded>,
    ) -> impl Future<Output = Result<()>> + Send;
    fn stop(
        definition: &PluginDefinition,
        authority: &PluginLifecycleAuthority<'_>,
        backend: &mut PluginBackend<Self::Guest, Self::Sidecar, Self::Embedded>,
    ) -> impl Future<Output = Result<()>> + Send;
}

/* === Implementations === */
impl<G: Send + 'static, S: Send + 'static, E: Send + 'static> Resource for Retiring<G, S, E> {
    fn finalize(&mut self) -> Finalize<'_> {
        (self.stop)(&self.definition, &self.scope, &mut self.backend)
    }
}

impl PluginLifecycleAuthority<'_> {
    pub fn owner(&self) -> &str {
        self.scope.owner()
    }

    pub fn generation(&self) -> PluginGeneration {
        self.scope.generation()
    }

    pub fn phase(&self) -> PluginLifecyclePhase {
        self.phase
    }

    pub fn backend_kind(&self) -> PluginKind {
        self.backend_kind
    }

    pub fn declaration(&self) -> Option<&harmos_contract::operations::lifecycle::Descriptor> {
        let pair = self.scope.lifecycle()?;
        Some(match self.phase {
            PluginLifecyclePhase::Initialize => &pair[0],
            PluginLifecyclePhase::Shutdown => &pair[1],
        })
    }

    pub fn matches_scope(
        &self,
        scope: &PluginScope,
        kind: PluginKind,
        phase: PluginLifecyclePhase,
    ) -> bool {
        self.scope.same_activation(scope) && self.backend_kind == kind && self.phase == phase
    }
}

impl From<PluginFailure> for Error {
    fn from(source: PluginFailure) -> Self {
        Self::PluginLifecycle {
            source: Box::new(source),
        }
    }
}

impl PluginReloadPlan {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn candidate(&self) -> &PluginDefinitions {
        &self.candidate
    }

    pub fn stopping(&self) -> &[PluginDefinition] {
        self.impact.stopping()
    }

    pub fn starting(&self) -> &[PluginDefinition] {
        self.impact.starting()
    }

    pub fn affected(&self) -> Vec<String> {
        affected(&self.impact)
            .into_iter()
            .map(|definition| definition.id)
            .collect()
    }
}

impl<G, S, E> PluginInstance<G, S, E> {
    fn ready(&self) -> bool {
        *self.state.borrow() == PluginState::Ready && self.scope.is_ready()
    }
}

impl<G: Send + 'static, S: Send + 'static, E: Send + 'static> PluginManager<G, S, E> {
    pub fn new(candidate: PluginDefinitions) -> Result<Self> {
        let identity = PluginGeneration::fresh()?;
        let definitions = by_id(&candidate.graph);
        Ok(Self {
            definitions,
            instances: BTreeMap::new(),
            router: Router::new(candidate.graph.clone()),
            graph: candidate.graph,
            failed: BTreeSet::new(),
            identity,
            revision: 0,
            events: broadcast::channel(EVENTS).0,
            retirement: None,
            cleanup: PluginCleanup::new(),
            executor: None,
            retain: retain_backend::<G, S, E>,
        })
    }

    pub fn resolve(&self) -> Result<PluginGraph> {
        let providers = self
            .graph
            .required_providers
            .iter()
            .flat_map(|(consumer, selected)| {
                selected
                    .iter()
                    .map(|(operation, provider)| super::OperationProvider {
                        consumer: consumer.clone(),
                        operation: operation.clone(),
                        provider: provider.clone(),
                    })
            });
        PluginGraph::resolve_with_providers(self.definitions.values().cloned(), providers)
            .map_err(|error| PluginFailure::Plan(error).into())
    }

    pub fn cleanup(&self) -> PluginCleanup {
        self.cleanup.clone()
    }

    pub fn router(&self) -> &Router {
        &self.router
    }

    pub fn definition(&self, id: &str) -> Option<&PluginDefinition> {
        self.definitions.get(id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &PluginDefinition> {
        self.definitions.values()
    }

    pub fn status(&self, id: &str) -> Option<PluginStatus> {
        let definition = self.definitions.get(id);
        let instance = self.instances.get(id).and_then(|instance| {
            let state = *instance.state.borrow();
            (state != PluginState::Registered).then_some((instance, state))
        });
        if definition.is_none() && instance.is_none() {
            return None;
        }
        Some(PluginStatus {
            id: id.to_owned(),
            definition: definition.cloned(),
            active_definition: instance.map(|(instance, _)| instance.definition.clone()),
            generation: instance.map(|(instance, _)| instance.generation),
            state: instance.map_or_else(
                || {
                    if self.failed.contains(id) && !self.instances.contains_key(id) {
                        PluginState::Failed
                    } else {
                        PluginState::Registered
                    }
                },
                |(instance, state)| {
                    if state == PluginState::Ready && !instance.scope.is_ready() {
                        PluginState::Failed
                    } else {
                        state
                    }
                },
            ),
        })
    }

    pub fn statuses(&self) -> impl Iterator<Item = PluginStatus> {
        let ids: BTreeSet<_> = self
            .definitions
            .keys()
            .chain(self.instances.keys())
            .collect();
        ids.into_iter().filter_map(|id| self.status(id))
    }

    pub fn dependencies(&self, id: &str) -> Result<Vec<String>> {
        if !self.definitions.contains_key(id) {
            return Err(unknown(id));
        }
        Ok(self.graph.dependencies(id).iter().cloned().collect())
    }

    pub fn dependents(&self, id: &str) -> Result<Vec<String>> {
        if !self.definitions.contains_key(id) {
            return Err(unknown(id));
        }
        Ok(self
            .definitions
            .values()
            .filter(|definition| self.graph.dependencies(&definition.id).contains(id))
            .map(|definition| definition.id.clone())
            .collect())
    }

    pub fn caller(&self, id: &str) -> Result<PluginCaller> {
        if !self
            .instances
            .get(id)
            .is_some_and(|instance| instance.ready())
        {
            return Err(Error::NotReady {
                domain: "plugin activation",
            });
        }
        self.router
            .caller(id)
            .map_err(|error| PluginFailure::Routing(error).into())
    }

    pub fn subscribe(&self) -> PluginEvents {
        PluginEvents {
            receiver: self.events.subscribe(),
        }
    }

    pub fn ready(&self) -> impl Iterator<Item = &str> {
        self.instances
            .iter()
            .filter(|(_, instance)| instance.ready())
            .map(|(id, _)| id.as_str())
    }

    pub fn plan_reload(&self, candidate: PluginDefinitions) -> Result<PluginReloadPlan> {
        if self.retirement.is_some() {
            return Err(Error::NotReady {
                domain: "plugin retirement",
            });
        }
        let current = self.router.graph();
        let mut changed = candidate.rebuilt.clone();
        for definition in self.graph.ordered().iter().chain(candidate.graph.ordered()) {
            if !current.same_contract(&definition.id, &candidate.graph)
                || !self
                    .instances
                    .get(&definition.id)
                    .is_some_and(|instance| instance.ready())
            {
                changed.insert(definition.id.clone());
            }
        }
        let impact = self
            .graph
            .reload(candidate.graph.clone(), changed)
            .map_err(PluginFailure::Plan)?;
        Ok(PluginReloadPlan {
            manager: self.identity,
            revision: self.revision,
            candidate,
            impact,
        })
    }

    pub async fn initialize<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        lifecycle: &mut L,
    ) -> Result<()> {
        self.resume_retirement().await?;
        self.resolve()?;
        let candidate = PluginDefinitions {
            graph: self.graph.clone(),
            rebuilt: BTreeSet::new(),
        };
        let plan = self.plan_reload(candidate)?;
        self.transition(plan, FailureScope::Instance, lifecycle)
            .await
    }

    pub async fn reload<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        candidate: PluginDefinitions,
        lifecycle: &mut L,
    ) -> Result<()> {
        self.resume_retirement().await?;
        let plan = self.plan_reload(candidate)?;
        self.apply_reload(plan, lifecycle).await
    }

    pub async fn apply_reload<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        plan: PluginReloadPlan,
        lifecycle: &mut L,
    ) -> Result<()> {
        self.transition(plan, FailureScope::Group, lifecycle).await
    }

    async fn transition<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        plan: PluginReloadPlan,
        failure_scope: FailureScope,
        lifecycle: &mut L,
    ) -> Result<()> {
        if plan.manager != self.identity {
            return Err(Error::InvalidInput {
                field: "plugin reload plan",
                reason: "plan belongs to another manager",
            });
        }
        if plan.revision != self.revision {
            return Err(PluginFailure::StalePlan {
                expected: plan.revision,
                actual: self.revision,
            }
            .into());
        }
        lifecycle.preflight(&plan.candidate)?;
        if plan.stopping().is_empty() && plan.starting().is_empty() {
            return Ok(());
        }
        let revision = self.next_revision()?;
        lifecycle
            .fence(
                match failure_scope {
                    FailureScope::Instance => PluginTransition::Initialize,
                    FailureScope::Group => PluginTransition::Reload,
                },
                &affected(&plan.impact),
            )
            .await
            .map_err(PluginFailure::Fence)?;

        // Advance before the first mutation, including cancellable teardown.
        self.revision = revision;
        let failures = self.stop(plan.stopping()).await;
        if !failures.is_empty() {
            return Err(PluginFailure::Cleanup(failures).into());
        }
        let current_graph = self.router.graph();
        let changed: Vec<_> = plan
            .candidate
            .graph
            .ordered()
            .iter()
            .filter(|definition| {
                self.definitions.get(&definition.id) != Some(*definition)
                    || !current_graph.same_contract(&definition.id, &plan.candidate.graph)
                    || plan.candidate.rebuilt.contains(&definition.id)
            })
            .map(|definition| definition.id.clone())
            .collect();
        self.router
            .reconfigure(plan.candidate.graph.clone())
            .map_err(PluginFailure::Routing)?;
        let removed: Vec<_> = self
            .definitions
            .keys()
            .filter(|id| {
                !plan
                    .candidate
                    .graph
                    .ordered()
                    .iter()
                    .any(|definition| definition.id == **id)
            })
            .cloned()
            .collect();
        self.graph = plan.candidate.graph;
        self.definitions = by_id(&self.graph);
        self.failed
            .retain(|id| self.definitions.contains_key(id) || self.instances.contains_key(id));
        for id in removed {
            self.emit(&id, None, PluginState::Registered, PluginEventKind::Removed);
        }
        for id in changed {
            self.notify(&id, PluginEventKind::DefinitionChanged);
        }

        let mut attempted = Vec::new();
        for definition in plan.impact.starting() {
            let generation = self.stage_generation::<L>(definition)?;
            attempted.push(definition.clone());
            let setup = self
                .initialize_generation(definition, generation, lifecycle)
                .await;
            if let Err(source) = setup {
                self.failed.insert(definition.id.clone());
                let stopping = match failure_scope {
                    FailureScope::Instance => vec![definition.clone()],
                    FailureScope::Group => {
                        attempted.reverse();
                        attempted
                    }
                };
                let cleanup = self.stop(&stopping).await;
                self.failed.insert(definition.id.clone());
                self.notify(&definition.id, PluginEventKind::Failed);
                return Err(PluginFailure::Start {
                    artifact: definition.id.clone(),
                    source,
                    cleanup,
                }
                .into());
            }
        }
        Ok(())
    }

    fn stage_generation<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        definition: &PluginDefinition,
    ) -> Result<PluginGeneration> {
        self.executor
            .get_or_insert_with(tokio::runtime::Handle::current);
        let generation = PluginGeneration::fresh()?;
        let scope = self
            .router
            .prepare_generation(definition, generation)
            .map_err(PluginFailure::Routing)?;
        self.failed.remove(&definition.id);
        self.instances.insert(
            definition.id.clone(),
            PluginInstance {
                definition: definition.clone(),
                generation,
                scope,
                state: watch::channel(PluginState::Initializing).0,
                backend: None,
                stop: stop_backend::<L>,
                cleanup: None,
            },
        );
        self.notify(&definition.id, PluginEventKind::Initializing);
        Ok(generation)
    }

    async fn initialize_generation<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        definition: &PluginDefinition,
        generation: PluginGeneration,
        lifecycle: &mut L,
    ) -> Result<()> {
        let setup = match lifecycle.prepare(definition) {
            Ok(backend) => {
                let kind = backend.kind();
                let instance = self
                    .instances
                    .get_mut(&definition.id)
                    .expect("tracked before backend preparation");
                instance.backend = Some(backend);
                if kind != definition.kind() {
                    Err(Error::InvalidInput {
                        field: "plugin backend",
                        reason: "backend kind does not match its definition",
                    })
                } else {
                    lifecycle
                        .start(
                            definition,
                            &self.router,
                            &PluginLifecycleAuthority {
                                scope: &instance.scope,
                                phase: PluginLifecyclePhase::Initialize,
                                backend_kind: kind,
                            },
                            instance.backend.as_mut().expect("retained before setup"),
                        )
                        .await
                }
            }
            Err(error) => Err(error),
        };
        let setup: Result<()> = setup.and_then(|()| {
            self.router
                .commit_generation(&definition.id, generation)
                .map_err(|error| PluginFailure::Routing(error).into())
        });
        setup?;
        self.instances
            .get_mut(&definition.id)
            .expect("tracked before initialization")
            .state
            .send_replace(PluginState::Ready);
        self.notify(&definition.id, PluginEventKind::Available);
        Ok(())
    }

    pub async fn shutdown<L: PluginLifecycle<Guest = G, Sidecar = S, Embedded = E>>(
        &mut self,
        lifecycle: &mut L,
    ) -> Result<()> {
        self.resume_retirement().await?;
        let stopping: Vec<_> = self
            .graph
            .ordered()
            .iter()
            .rev()
            .filter(|definition| self.instances.contains_key(&definition.id))
            .cloned()
            .collect();
        let revision = self.next_revision()?;
        lifecycle
            .fence(PluginTransition::Shutdown, &stopping)
            .await
            .map_err(PluginFailure::Fence)?;
        self.revision = revision;
        let failures = self.stop(&stopping).await;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(PluginFailure::Cleanup(failures).into())
        }
    }

    async fn stop(&mut self, stopping: &[PluginDefinition]) -> Vec<PluginCleanupFailure> {
        if self.retirement.is_some() {
            let failures = self.join_retirement().await;
            if !failures.is_empty() {
                return failures;
            }
        }
        let selected: Vec<_> = stopping
            .iter()
            .filter_map(|definition| {
                self.instances
                    .get(&definition.id)
                    .map(|instance| (definition.id.clone(), instance.generation))
            })
            .collect();
        if selected.is_empty() {
            return Vec::new();
        }
        let schedule = self
            .graph
            .topology
            .retirement(selected.iter().map(|(id, _)| id.clone()).collect());
        let mut owners = BTreeMap::new();
        for (id, generation) in &selected {
            let instance = self.instances.get_mut(id).expect("selected original owner");
            (self.retain)(instance);
            owners.insert(
                id.clone(),
                retirement::Owner {
                    generation: *generation,
                    state: instance.state.clone(),
                    cleanup: instance.cleanup.clone(),
                },
            );
        }
        let outcomes = Arc::new(Mutex::new(BTreeMap::new()));
        let driver = retirement::Driver {
            previous: None,
            schedule,
            owners,
            router: self.router.clone(),
            events: self.events.clone(),
            revision: self.revision,
            outcomes: outcomes.clone(),
        };
        self.retirement = Some(retirement::Wait {
            cleanup: Arc::new(Cleanup::new(driver)),
            selected,
            outcomes,
        });
        self.join_retirement().await
    }

    async fn resume_retirement(&mut self) -> Result<()> {
        if self.retirement.is_none() {
            return Ok(());
        }
        let failures = self.join_retirement().await;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(PluginFailure::Cleanup(failures).into())
        }
    }

    async fn join_retirement(&mut self) -> Vec<PluginCleanupFailure> {
        let retained = self.retirement.as_ref().expect("original retirement pass");
        let result = retained.cleanup.settle().await;
        let mut failures = Vec::new();
        if let Err(source) = result {
            for (id, _) in &retained.selected {
                failures.push(PluginCleanupFailure {
                    artifact: id.clone(),
                    error: Error::TaskFailed {
                        domain: "plugin retirement",
                        source: source.clone(),
                    },
                });
            }
            return failures;
        }
        let retained = self.retirement.take().expect("joined original pass");
        let mut outcomes = std::mem::take(
            &mut *retained
                .outcomes
                .lock()
                .expect("retirement outcomes are healthy"),
        );
        for (id, generation) in retained.selected {
            let Some(result) = outcomes.remove(&id) else {
                continue;
            };
            assert_eq!(
                self.instances[&id].generation, generation,
                "retirement belongs to the original generation"
            );
            match result {
                Ok(()) => {
                    self.instances.remove(&id);
                    self.failed.remove(&id);
                }
                Err(source) => {
                    self.failed.insert(id.clone());
                    failures.push(PluginCleanupFailure {
                        artifact: id,
                        error: Error::TaskFailed {
                            domain: "plugin cleanup",
                            source,
                        },
                    });
                }
            }
        }
        failures
    }

    fn next_revision(&self) -> Result<u64> {
        self.revision
            .checked_add(1)
            .ok_or(Error::ResourceExhausted {
                resource: "plugin lifecycle revisions",
            })
    }

    fn notify(&self, id: &str, kind: PluginEventKind) {
        let instance = self.instances.get(id);
        let state = instance.map_or_else(
            || {
                if self.failed.contains(id) {
                    PluginState::Failed
                } else {
                    PluginState::Registered
                }
            },
            |instance| *instance.state.borrow(),
        );
        self.emit(
            id,
            instance.map(|instance| instance.generation),
            state,
            kind,
        );
    }

    fn emit(
        &self,
        id: &str,
        generation: Option<PluginGeneration>,
        state: PluginState,
        kind: PluginEventKind,
    ) {
        let _ = self.events.send(PluginEvent::Changed {
            id: id.to_owned(),
            generation,
            state,
            revision: self.revision,
            kind,
        });
    }
}

impl<G, S, E> Drop for PluginManager<G, S, E> {
    fn drop(&mut self) {
        if self.instances.is_empty() {
            self.cleanup.settled();
            return;
        }
        let mut owners = BTreeMap::new();
        for (id, instance) in &mut self.instances {
            (self.retain)(instance);
            owners.insert(
                id.clone(),
                retirement::Owner {
                    generation: instance.generation,
                    state: instance.state.clone(),
                    cleanup: instance.cleanup.clone(),
                },
            );
        }
        let _execution = self
            .executor
            .as_ref()
            .expect("active instances retain their executor")
            .enter();
        self.cleanup.start(retirement::Closing {
            topology: self.graph.topology.clone(),
            owners,
            previous: self.retirement.take().map(Arc::new),
            router: self.router.clone(),
            events: self.events.clone(),
            revision: self.revision,
        });
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn retain_backend<G: Send + 'static, S: Send + 'static, E: Send + 'static>(
    instance: &mut PluginInstance<G, S, E>,
) {
    if instance.cleanup.is_none()
        && let Some(backend) = instance.backend.take()
    {
        let original = Retiring {
            definition: instance.definition.clone(),
            scope: instance.scope.clone(),
            backend,
            stop: instance.stop,
        };
        let resource: Box<dyn Resource> = Box::new(original);
        instance.cleanup = Some(Arc::new(Cleanup::new(resource)));
    }
}

fn stop_backend<'a, L: PluginLifecycle>(
    definition: &'a PluginDefinition,
    scope: &'a PluginScope,
    backend: &'a mut PluginBackend<L::Guest, L::Sidecar, L::Embedded>,
) -> Finalize<'a> {
    Box::pin(async move {
        let authority = PluginLifecycleAuthority {
            scope,
            phase: PluginLifecyclePhase::Shutdown,
            backend_kind: backend.kind(),
        };
        L::stop(definition, &authority, backend)
            .await
            .map_err(Arc::new)
    })
}

fn by_id(graph: &PluginGraph) -> BTreeMap<String, PluginDefinition> {
    graph
        .ordered()
        .iter()
        .map(|definition| (definition.id.clone(), definition.clone()))
        .collect()
}

fn unknown(id: &str) -> Error {
    Error::NotRegistered {
        domain: "plugins",
        name: id.to_owned(),
    }
}

fn affected(impact: &ReloadImpact) -> Vec<PluginDefinition> {
    let mut definitions = BTreeMap::new();
    for definition in impact.stopping().iter().chain(impact.starting()) {
        definitions.insert(definition.id.clone(), definition.clone());
    }
    definitions.into_values().collect()
}

/* === Tests === */
