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
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::{PluginGeneration, PluginGraph, PluginScope};
use harmos_domain::Refusal;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone)]
pub struct Router(Arc<Registry>);

struct Registry {
    state: Mutex<RegistryState>,
}

struct RegistryState {
    graph: PluginGraph,
    generations: BTreeMap<String, PluginScope>,
    instances: BTreeMap<String, Weak<Live>>,
}

struct Live {
    generation: PluginGeneration,
    scope: Option<PluginScope>,
    alive: watch::Sender<bool>,
}

pub struct PluginEndpoint {
    registry: Weak<Registry>,
    live: Arc<Live>,
}

#[derive(Clone)]
pub struct PluginCaller {
    generation: PluginGeneration,
    registry: Weak<Registry>,
    live: Weak<Live>,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("plugin `{owner}` is absent from the admitted graph")]
    UnknownOwner { owner: String },
    #[error("plugin `{0}` is already serving or admitted")]
    Occupied(String),
    #[error("plugin `{owner}` has not bound its complete admitted operation inventory")]
    IncompleteOperations { owner: String },
    #[error("plugin `{owner}` has not retained its complete admitted stream inventory")]
    IncompleteStreams { owner: String },
    #[error("plugin `{owner}` has not retained its complete admitted listener inventory")]
    IncompleteListeners { owner: String },
    #[error("the plugin instance is unavailable")]
    Unavailable,
    #[error("the plugin request queue is full")]
    Busy,
    #[error("the route deadline expired")]
    Deadline,
    #[error(transparent)]
    Refused(#[from] Refusal),
}

/* === Traits === */

/* === Implementations === */
impl DispatchError {
    pub fn code(&self) -> i64 {
        match self {
            Self::Refused(refusal) => refusal.code(),
            Self::UnknownOwner { .. }
            | Self::IncompleteOperations { .. }
            | Self::IncompleteStreams { .. }
            | Self::IncompleteListeners { .. } => -32_604,
            Self::Busy => -32_605,
            Self::Unavailable | Self::Occupied(_) => -32_606,
            Self::Deadline => -32_607,
        }
    }

    pub fn into_refusal(self) -> Refusal {
        match self {
            Self::Refused(refusal) => refusal,
            error => Refusal::new(error.code(), error),
        }
    }
}

impl Router {
    pub fn new(graph: PluginGraph) -> Self {
        Self(Arc::new(Registry { state: Mutex::new(RegistryState { graph, generations: BTreeMap::new(), instances: BTreeMap::new() }) }))
    }

    pub fn graph(&self) -> PluginGraph {
        self.0.state.lock().expect("plugin registry lock").graph.clone()
    }

    pub fn enter(&self, owner: &str) -> Result<PluginEndpoint, DispatchError> {
        let mut state = self.0.state.lock().expect("plugin registry lock");
        if !state.graph.knows(owner) {
            return Err(DispatchError::UnknownOwner { owner: owner.to_owned() });
        }

        if state.instances.get(owner).and_then(Weak::upgrade).is_some_and(|live| *live.alive.borrow()) {
            return Err(DispatchError::Occupied(owner.to_owned()));
        }

        let generation = match state.generations.get(owner) {
            Some(scope) => scope.generation(),
            None => PluginGeneration::fresh().map_err(|_| DispatchError::Unavailable)?,
        };
        let live = Arc::new(Live { generation, scope: state.generations.get(owner).cloned(), alive: watch::channel(true).0 });
        state.instances.insert(owner.to_owned(), Arc::downgrade(&live));

        Ok(PluginEndpoint { registry: Arc::downgrade(&self.0), live })
    }
}

impl Router {
    pub(super) fn prepare_generation(
        &self, definition: &super::PluginDefinition, generation: PluginGeneration,
    ) -> Result<PluginScope, DispatchError> {
        let owner = definition.id.as_str();
        let mut state = self.0.state.lock().expect("plugin registry lock");
        if state.instances.get(owner).and_then(Weak::upgrade).is_some_and(|live| *live.alive.borrow()) {
            return Err(DispatchError::Occupied(owner.to_owned()));
        }
        let scope = PluginScope::with_dependencies(owner, generation, &definition.requires)
            .with_requirements(
                definition.contributions.required_contracts.as_ref(),
                state.graph.required_providers(owner),
                state.graph.dependencies(owner),
            )
            .with_operations(&definition.contributions.operations)
            .with_wire_contracts(&definition.contributions.wire_contracts)
            .with_observers(&definition.contributions.observers)
            .with_streams(&definition.contributions.streams)
            .with_listeners(&definition.contributions.listeners)
            .with_lifecycle(definition.contributions.lifecycle.as_ref());
        state.generations.insert(owner.to_owned(), scope.clone());
        Ok(scope)
    }

    pub(super) fn commit_generation(&self, owner: &str, generation: PluginGeneration) -> Result<(), DispatchError> {
        let mut state = self.0.state.lock().expect("plugin registry lock");
        let admission =
            state.generations.get_mut(owner).filter(|scope| scope.generation() == generation).ok_or(DispatchError::Unavailable)?;
        if admission.commit() {
            Ok(())
        } else if admission.is_closed() {
            Err(DispatchError::Unavailable)
        } else if !admission.listeners_bound() {
            Err(DispatchError::IncompleteListeners { owner: owner.to_owned() })
        } else if !admission.streams_bound() {
            Err(DispatchError::IncompleteStreams { owner: owner.to_owned() })
        } else {
            Err(DispatchError::IncompleteOperations { owner: owner.to_owned() })
        }
    }

    pub(super) fn caller(&self, owner: &str) -> Result<PluginCaller, DispatchError> {
        let state = self.0.state.lock().expect("plugin registry lock");
        let live =
            state.instances.get(owner).and_then(Weak::upgrade).filter(|live| *live.alive.borrow()).ok_or(DispatchError::Unavailable)?;
        Ok(PluginCaller { generation: live.generation, registry: Arc::downgrade(&self.0), live: Arc::downgrade(&live) })
    }

    pub(super) fn quiesce(&self, owner: &str) {
        let state = self.0.state.lock().expect("plugin registry lock");
        if let Some(scope) = state.generations.get(owner) {
            scope.close();
        }
    }

    pub(super) fn revoke(&self, owner: &str) {
        let state = self.0.state.lock().expect("plugin registry lock");
        if let Some(scope) = state.generations.get(owner) {
            scope.close();
        }
        if let Some(live) = state.instances.get(owner).and_then(Weak::upgrade) {
            live.alive.send_replace(false);
        }
    }

    pub fn reconfigure(&self, graph: PluginGraph) -> Result<(), DispatchError> {
        let mut state = self.0.state.lock().expect("plugin registry lock");
        for (owner, live) in &state.instances {
            if live.upgrade().is_some_and(|live| *live.alive.borrow()) && !state.graph.same_contract(owner, &graph) {
                return Err(DispatchError::Occupied(owner.clone()));
            }
        }
        state.instances.retain(|_, live| live.upgrade().is_some_and(|live| *live.alive.borrow()));
        state.generations.retain(|owner, _| graph.knows(owner));
        state.graph = graph;
        Ok(())
    }
}

impl PluginEndpoint {
    pub fn scope(&self) -> Option<&PluginScope> {
        self.live.scope.as_ref()
    }

    #[cfg(all(feature = "plugin-guest", not(target_arch = "wasm32")))]
    pub(crate) fn revocation(&self) -> watch::Sender<bool> {
        self.live.alive.clone()
    }

    pub fn caller(&self) -> PluginCaller {
        PluginCaller { generation: self.live.generation, registry: self.registry.clone(), live: Arc::downgrade(&self.live) }
    }

    pub fn quiesce(&mut self) {
        if let Some(scope) = &self.live.scope {
            scope.close();
        }
    }
}

impl Drop for PluginEndpoint {
    fn drop(&mut self) {
        self.live.alive.send_replace(false);
    }
}

impl PluginCaller {
    pub fn generation(&self) -> PluginGeneration {
        self.generation
    }

    pub(crate) fn operation_scope(&self) -> Result<PluginScope, DispatchError> {
        self.registry.upgrade().ok_or(DispatchError::Unavailable)?;
        let live = self.live.upgrade().ok_or(DispatchError::Unavailable)?;
        if live.generation != self.generation || !*live.alive.borrow() {
            return Err(DispatchError::Unavailable);
        }
        live.scope.as_ref().cloned().ok_or(DispatchError::Unavailable)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
