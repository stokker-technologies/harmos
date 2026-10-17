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
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, OnceLock, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::PluginGeneration;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct PluginScope {
    owner: Arc<str>,
    generation: PluginGeneration,
    #[cfg(any(
        feature = "plugin-sidecar",
        all(feature = "plugin-guest", not(target_arch = "wasm32"))
    ))]
    generation_context: Arc<OnceLock<[u8; 32]>>,
    phase: Arc<watch::Sender<Phase>>,
    operation_registry: Arc<OnceLock<Weak<crate::operations::Registry>>>,
    dependencies: Arc<BTreeSet<String>>,
    operations: Arc<BTreeMap<crate::Identifier, crate::operations::Definition>>,
    wire_contracts: Arc<BTreeMap<crate::Identifier, crate::operations::Contract>>,
    required_contracts: Arc<Option<Vec<harmos_domain::inventory::contracts::Required>>>,
    required_providers: Arc<BTreeMap<String, String>>,
    observers: Arc<[harmos_domain::inventory::observers::Declaration]>,
    observe_contract: Arc<Option<crate::operations::Contract>>,
    streams: Arc<[harmos_domain::StreamDeclaration]>,
    listeners: Arc<[harmos_domain::listeners::Definition]>,
    lifecycle: Arc<Option<[harmos_domain::operations::lifecycle::Descriptor; 2]>>,
    activation: Arc<Mutex<Option<Weak<crate::streams::Activation>>>>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Staged {
        operations_bound: bool,
        operations_abandoned: bool,
        streams_bound: bool,
        streams_abandoned: bool,
        record_listeners_bound: bool,
        stream_listeners_bound: bool,
    },
    Ready,
    Closed {
        committed: bool,
    },
}

/* === Traits === */

/* === Implementations === */
impl PluginScope {
    pub(crate) fn same_activation(&self, other: &Self) -> bool {
        self.generation == other.generation && Arc::ptr_eq(&self.phase, &other.phase)
    }

    #[cfg(any(
        feature = "plugin-sidecar",
        all(feature = "plugin-guest", not(target_arch = "wasm32"))
    ))]
    pub(crate) fn issue_generation_context(&self) -> Result<[u8; 32]> {
        if self.is_closed() {
            return Err(Error::Closing {
                domain: "plugin generation",
            });
        }
        if let Some(context) = self.generation_context.get() {
            return Ok(*context);
        }
        let mut context = [0; 32];
        getrandom::fill(&mut context).map_err(|source| Error::OperationFailed {
            operation: "plugin generation context".to_owned(),
            source: Arc::new(source),
        })?;
        if context == [0; 32] {
            return Err(Error::InvalidInput {
                field: "plugin generation context",
                reason: "zero random context",
            });
        }
        let _ = self.generation_context.set(context);
        Ok(*self
            .generation_context
            .get()
            .expect("generation context issued once"))
    }

    #[cfg(any(
        feature = "plugin-sidecar",
        all(feature = "plugin-guest", not(target_arch = "wasm32"))
    ))]
    pub(crate) fn accepts_generation_context(&self, context: &[u8]) -> bool {
        !self.is_closed()
            && self
                .generation_context
                .get()
                .is_some_and(|original| original.as_slice() == context)
    }

    pub(super) fn new(owner: &str, generation: PluginGeneration) -> Self {
        Self {
            owner: Arc::from(owner),
            generation,
            #[cfg(any(
                feature = "plugin-sidecar",
                all(feature = "plugin-guest", not(target_arch = "wasm32"))
            ))]
            generation_context: Arc::new(OnceLock::new()),
            phase: Arc::new(
                watch::channel(Phase::Staged {
                    operations_bound: true,
                    operations_abandoned: false,
                    streams_bound: true,
                    streams_abandoned: false,
                    record_listeners_bound: true,
                    stream_listeners_bound: true,
                })
                .0,
            ),
            operation_registry: Arc::new(OnceLock::new()),
            dependencies: Arc::new(BTreeSet::new()),
            operations: Arc::new(BTreeMap::new()),
            wire_contracts: Arc::new(BTreeMap::new()),
            required_contracts: Arc::new(None),
            required_providers: Arc::new(BTreeMap::new()),
            observers: Arc::from([]),
            observe_contract: Arc::new(None),
            streams: Arc::from([]),
            listeners: Arc::from([]),
            lifecycle: Arc::new(None),
            activation: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) fn with_dependencies(
        owner: &str,
        generation: PluginGeneration,
        requirements: &[super::Requirement],
    ) -> Self {
        let mut scope = Self::new(owner, generation);
        scope.dependencies = Arc::new(
            requirements
                .iter()
                .map(|required| required.id.clone())
                .collect(),
        );
        scope
    }

    pub(super) fn with_lifecycle(
        mut self,
        pair: Option<&[harmos_domain::operations::lifecycle::Descriptor; 2]>,
    ) -> Self {
        self.lifecycle = Arc::new(pair.cloned());
        self
    }

    pub fn lifecycle(&self) -> Option<&[harmos_domain::operations::lifecycle::Descriptor; 2]> {
        self.lifecycle.as_ref().as_ref()
    }

    pub(super) fn with_operations(mut self, operations: &[crate::operations::Definition]) -> Self {
        self.operations = Arc::new(
            operations
                .iter()
                .map(|operation| (operation.id.clone(), operation.clone()))
                .collect(),
        );
        self.phase.send_if_modified(|phase| {
            let Phase::Staged {
                operations_bound, ..
            } = phase
            else {
                return false;
            };
            *operations_bound = operations.is_empty();
            true
        });
        self
    }

    pub(super) fn with_observers(
        mut self,
        observers: &[harmos_domain::inventory::observers::Declaration],
    ) -> Self {
        self.observers = Arc::from(observers.to_vec());
        // A Sidecar That Declares Observers Serves The Built-In Observe Control.
        self.observe_contract = Arc::new((!observers.is_empty()).then(|| {
            harmos_domain::observers::contract().expect("the built-in observe control is valid")
        }));
        self
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn observers(&self) -> &[harmos_domain::inventory::observers::Declaration] {
        &self.observers
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn observe_contract(&self) -> Option<&crate::operations::Contract> {
        self.observe_contract.as_ref().as_ref()
    }

    pub(super) fn with_wire_contracts(mut self, contracts: &[crate::operations::Contract]) -> Self {
        self.wire_contracts = Arc::new(
            contracts
                .iter()
                .map(|contract| (contract.operation().id.clone(), contract.clone()))
                .collect(),
        );
        self
    }

    pub(super) fn with_requirements(
        mut self,
        contracts: Option<&Vec<harmos_domain::inventory::contracts::Required>>,
        providers: &BTreeMap<crate::Identifier, String>,
        dependencies: &BTreeSet<String>,
    ) -> Self {
        self.required_contracts = Arc::new(contracts.cloned());
        self.required_providers = Arc::new(
            providers
                .iter()
                .map(|(id, owner)| (id.to_string(), owner.clone()))
                .collect(),
        );
        self.dependencies = Arc::new(dependencies.clone());
        self
    }

    pub(crate) fn required_contracts(
        &self,
    ) -> Option<&[harmos_domain::inventory::contracts::Required]> {
        self.required_contracts.as_ref().as_deref()
    }

    pub fn requirement(
        &self,
        id: &crate::Identifier,
    ) -> Option<&harmos_domain::inventory::contracts::Required> {
        self.required_contracts()?
            .iter()
            .find(|required| required.contract.operation().id == *id)
    }

    pub(crate) fn required_provider(&self, id: &str) -> Option<&str> {
        self.required_providers.get(id).map(String::as_str)
    }

    pub(crate) fn allows_operation(
        &self,
        owner: &str,
        contract: Option<&crate::operations::Contract>,
        id: &crate::Identifier,
        host: bool,
    ) -> bool {
        match self.required_contracts() {
            None => self.allows(owner),
            Some(required) => {
                let provider = match self.required_providers.get(id.as_str()) {
                    Some(selected) => selected == owner,
                    None => host,
                };
                provider
                    && contract.is_some_and(|actual| {
                        required.iter().any(|expected| expected.contract == *actual)
                    })
            }
        }
    }

    pub(crate) fn validate_wire_contracts<'a>(
        &self,
        contracts: impl IntoIterator<Item = &'a crate::operations::Contract>,
    ) -> Result<()> {
        let actual: BTreeMap<_, _> = contracts
            .into_iter()
            .map(|contract| (&contract.operation().id, contract))
            .collect();
        if self
            .wire_contracts
            .iter()
            .any(|(id, expected)| actual.get(id).is_none_or(|actual| *actual != expected))
        {
            return Err(Error::InvalidInput {
                field: "operation contract",
                reason: "wire bindings differ from the admitted plugin inventory",
            });
        }
        Ok(())
    }

    pub(super) fn with_streams(mut self, streams: &[harmos_domain::StreamDeclaration]) -> Self {
        let mut frozen = streams.to_vec();
        frozen.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        self.streams = Arc::from(frozen);
        self.phase.send_if_modified(|phase| {
            let Phase::Staged { streams_bound, .. } = phase else {
                return false;
            };
            *streams_bound = streams.is_empty();
            true
        });
        self
    }

    pub(super) fn with_listeners(
        mut self,
        listeners: &[harmos_domain::listeners::Definition],
    ) -> Self {
        self.listeners = Arc::from(listeners.to_vec());
        self.phase.send_if_modified(|phase| {
            let Phase::Staged {
                record_listeners_bound,
                stream_listeners_bound,
                ..
            } = phase
            else {
                return false;
            };
            *record_listeners_bound = !listeners.iter().any(|listener| {
                matches!(
                    listener.source(),
                    harmos_domain::listeners::Source::Record { .. }
                )
            });
            *stream_listeners_bound = !listeners.iter().any(|listener| {
                matches!(
                    listener.source(),
                    harmos_domain::listeners::Source::Stream { .. }
                )
            });
            true
        });
        self
    }

    pub fn listeners(&self) -> &[harmos_domain::listeners::Definition] {
        &self.listeners
    }

    pub(crate) fn listeners_bound(&self) -> bool {
        !matches!(
            *self.phase.borrow(),
            Phase::Staged {
                record_listeners_bound: false,
                ..
            } | Phase::Staged {
                stream_listeners_bound: false,
                ..
            }
        )
    }

    pub(crate) fn bind_record_listeners(&self) -> Result<()> {
        let mut bound = false;
        self.phase.send_if_modified(|phase| {
            let Phase::Staged {
                record_listeners_bound: false,
                ..
            } = phase
            else {
                return false;
            };
            bound = true;
            if let Phase::Staged {
                record_listeners_bound,
                ..
            } = phase
            {
                *record_listeners_bound = true;
            }
            true
        });
        if bound {
            Ok(())
        } else {
            Err(Error::Closing {
                domain: "record listener staging",
            })
        }
    }

    pub(crate) fn bind_stream_listeners(&self) -> Result<()> {
        let mut bound = false;
        self.phase.send_if_modified(|phase| {
            let Phase::Staged {
                stream_listeners_bound: false,
                ..
            } = phase
            else {
                return false;
            };
            bound = true;
            if let Phase::Staged {
                stream_listeners_bound,
                ..
            } = phase
            {
                *stream_listeners_bound = true;
            }
            true
        });
        if bound {
            Ok(())
        } else {
            Err(Error::Closing {
                domain: "stream listener staging",
            })
        }
    }

    pub(crate) fn close_listeners(&self) {
        if !self.listeners.is_empty() {
            self.close();
        }
    }

    pub(crate) async fn closed(&self) {
        let mut phase = self.phase.subscribe();
        loop {
            if matches!(*phase.borrow_and_update(), Phase::Closed { .. }) {
                return;
            }
            if phase.changed().await.is_err() {
                return;
            }
        }
    }

    pub(crate) fn matches_streams(&self, definitions: &[crate::streams::Definition]) -> bool {
        definitions.len() == self.streams.len()
            && definitions.iter().all(|definition| {
                self.streams
                    .binary_search_by(|declared| declared.id.as_str().cmp(definition.id()))
                    .ok()
                    .is_some_and(|index| {
                        let declared = &self.streams[index];
                        definition.version() == declared.schema
                            && definition.window() == declared.window
                            && definition.metadata() == &declared.metadata
                            && definition.row_metadata() == declared.row_metadata
                            && definition.schema() == Some(&declared.row)
                    })
            })
    }

    pub fn publication(
        &self,
        id: &str,
        schema: u32,
    ) -> crate::Result<Arc<crate::streams::PublicationLease>> {
        if self.is_closed() {
            return Err(crate::Error::Closing {
                domain: "publication generation",
            });
        }
        self.selected_publication(id, schema)
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn publication_in(
        &self,
        context: &crate::operations::Context<'_>,
        id: &str,
        schema: u32,
    ) -> crate::Result<Arc<crate::streams::PublicationLease>> {
        if self.is_closed() && !(self.has_committed() && context.retains_publication(self)) {
            return Err(crate::Error::Closing {
                domain: "publication generation",
            });
        }
        self.selected_publication(id, schema)
    }

    fn selected_publication(
        &self,
        id: &str,
        schema: u32,
    ) -> crate::Result<Arc<crate::streams::PublicationLease>> {
        let selected = self
            .streams
            .binary_search_by(|stream| stream.id.as_str().cmp(id))
            .ok()
            .map(|index| &self.streams[index]);
        if !selected.is_some_and(|stream| stream.schema == schema) {
            return Err(crate::Error::InvalidInput {
                field: "publication stream",
                reason: "stream or schema differs from the frozen generation inventory",
            });
        }
        let activation = self
            .activation
            .lock()
            .expect("plugin stream binding poisoned")
            .as_ref()
            .and_then(Weak::upgrade)
            .ok_or(crate::Error::NotReady {
                domain: "publication generation",
            })?;
        activation
            .leases()
            .iter()
            .find(|lease| {
                let definition = lease.stream().definition();
                definition.id() == id && definition.version() == schema
            })
            .cloned()
            .ok_or(crate::Error::NotReady {
                domain: "publication generation",
            })
    }

    pub(crate) fn attach_streams(&self, activation: &Arc<crate::streams::Activation>) {
        *self
            .activation
            .lock()
            .expect("plugin stream binding poisoned") = Some(Arc::downgrade(activation));
    }

    pub(crate) fn prepare_publication<T>(
        &self,
        prepare: impl FnOnce(bool) -> std::result::Result<T, crate::streams::StreamPublicationError>,
    ) -> std::result::Result<T, crate::streams::StreamPublicationError> {
        self.prepare_publication_in(None, prepare)
    }

    pub(crate) fn prepare_publication_in<T>(
        &self,
        context: Option<&crate::operations::Context<'_>>,
        prepare: impl FnOnce(bool) -> std::result::Result<T, crate::streams::StreamPublicationError>,
    ) -> std::result::Result<T, crate::streams::StreamPublicationError> {
        // Validate Before The Phase Lock: Descendant Source Checks Read This Phase.
        let lifecycle = context.is_some_and(|context| context.retains_publication(self));
        let mut result = Err(crate::streams::StreamPublicationError::Revoked);
        self.phase.send_if_modified(|phase| {
            if !matches!(
                *phase,
                Phase::Closed { .. }
                    | Phase::Staged {
                        streams_abandoned: true,
                        ..
                    }
            ) || (lifecycle && *phase == Phase::Closed { committed: true })
            {
                let ready = matches!(*phase, Phase::Ready | Phase::Closed { committed: true });
                result = prepare(ready);
            }
            false
        });
        result
    }

    pub(crate) fn bind_streams(&self) -> bool {
        let mut bound = false;
        self.phase.send_if_modified(|phase| {
            let Phase::Staged {
                streams_bound,
                streams_abandoned: false,
                ..
            } = phase
            else {
                return false;
            };
            bound = true;
            let changed = !*streams_bound;
            *streams_bound = true;
            changed
        });
        // A transport restart can renew the same inventory after commit.
        bound || self.is_ready()
    }

    pub(crate) fn unbind_streams(&self) {
        if self.streams.is_empty() {
            return;
        }
        self.phase.send_if_modified(|phase| {
            let Phase::Staged {
                streams_bound,
                streams_abandoned,
                ..
            } = phase
            else {
                return false;
            };
            if !*streams_bound {
                return false;
            }
            let changed = *streams_bound || !*streams_abandoned;
            *streams_bound = false;
            *streams_abandoned = true;
            changed
        });
        if !self.has_committed() {
            let activation = self
                .activation
                .lock()
                .expect("plugin stream binding poisoned")
                .as_ref()
                .and_then(Weak::upgrade);
            if let Some(activation) = activation {
                activation.abort();
            }
        }
    }

    pub(crate) fn streams_bound(&self) -> bool {
        !matches!(
            *self.phase.borrow(),
            Phase::Staged {
                streams_bound: false,
                ..
            }
        )
    }

    pub(crate) fn validate_operations<'a>(
        &self,
        operations: impl IntoIterator<Item = &'a crate::operations::Definition>,
    ) -> Result<()> {
        // The built-in observe control is served beside the admitted inventory, never inside it.
        let observe = self
            .observe_contract
            .as_ref()
            .as_ref()
            .map(|contract| contract.operation());
        let actual: BTreeMap<_, _> = operations
            .into_iter()
            .filter(|operation| Some(*operation) != observe)
            .map(|operation| (&operation.id, operation))
            .collect();
        if actual.len() != self.operations.len()
            || self
                .operations
                .iter()
                .any(|(id, declared)| actual.get(id).is_none_or(|actual| *actual != declared))
        {
            return Err(Error::InvalidInput {
                field: "operation catalog",
                reason: "bindings differ from the admitted plugin inventory",
            });
        }
        Ok(())
    }

    pub(crate) fn bind_operations(&self) -> Result<()> {
        let mut bound = false;
        self.phase.send_if_modified(|phase| {
            if let Phase::Staged {
                operations_bound,
                operations_abandoned: false,
                ..
            } = phase
            {
                bound = true;
                let changed = !*operations_bound;
                *operations_bound = true;
                changed
            } else {
                false
            }
        });
        if !bound {
            return Err(Error::Closing {
                domain: "plugin operation staging",
            });
        }
        Ok(())
    }

    pub(crate) fn unbind_operations(&self) {
        self.phase.send_if_modified(|phase| {
            if let Phase::Staged {
                operations_bound,
                operations_abandoned,
                ..
            } = phase
            {
                if self.operations.is_empty() {
                    return false;
                }
                let changed = *operations_bound || !*operations_abandoned;
                *operations_bound = false;
                *operations_abandoned = true;
                changed
            } else {
                false
            }
        });
    }

    pub(crate) fn allows(&self, owner: &str) -> bool {
        self.dependencies.contains(owner)
    }

    pub(crate) fn owner_identity(&self) -> Arc<str> {
        self.owner.clone()
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn generation(&self) -> PluginGeneration {
        self.generation
    }

    pub fn is_ready(&self) -> bool {
        *self.phase.borrow() == Phase::Ready
    }

    pub async fn ready(&self) -> Result<()> {
        let mut phase = self.phase.subscribe();
        loop {
            match *phase.borrow_and_update() {
                Phase::Ready => return Ok(()),
                Phase::Closed { .. } => {
                    return Err(Error::Closing {
                        domain: "plugin scope",
                    });
                }
                Phase::Staged { .. } => {}
            }
            phase.changed().await.map_err(|_| Error::Closing {
                domain: "plugin scope",
            })?;
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        matches!(*self.phase.borrow(), Phase::Closed { .. })
    }

    pub(crate) fn bind_operation_registry(
        &self,
        registry: &Arc<crate::operations::Registry>,
    ) -> Result<()> {
        let original = self
            .operation_registry
            .get_or_init(|| Arc::downgrade(registry));
        if !Weak::ptr_eq(original, &Arc::downgrade(registry)) {
            return Err(Error::InvalidInput {
                field: "plugin operation registry",
                reason: "generation already belongs to another registry",
            });
        }
        Ok(())
    }

    pub(super) fn commit(&self) -> bool {
        // Retain The Original Registry Gate Before Taking Visibility Or Phase Locks.
        let registry = match self.operation_registry.get() {
            Some(original) => {
                let Some(registry) = original.upgrade() else {
                    return false;
                };
                Some(registry)
            }
            None => None,
        };
        let mut admission = match &registry {
            Some(registry) => match registry.generation_commit() {
                Ok(guard) => Some(guard),
                Err(_) => return false,
            },
            None => None,
        };
        let mut binding = Some(
            self.activation
                .lock()
                .expect("plugin stream binding poisoned"),
        );
        let activation = binding
            .as_ref()
            .and_then(|binding| binding.as_ref().and_then(Weak::upgrade));
        let visibility = activation
            .as_ref()
            .and_then(|activation| activation.visibility());
        let mut guard = visibility
            .as_ref()
            .map(|visibility| visibility.write().expect("stream visibility poisoned"));
        let mut committed = false;
        self.phase.send_if_modified(|phase| {
            let changed = (|| {
                if matches!(
                    *phase,
                    Phase::Closed { .. }
                        | Phase::Staged {
                            operations_bound: false,
                            ..
                        }
                        | Phase::Staged {
                            streams_bound: false,
                            ..
                        }
                        | Phase::Staged {
                            record_listeners_bound: false,
                            ..
                        }
                        | Phase::Staged {
                            stream_listeners_bound: false,
                            ..
                        }
                ) {
                    return false;
                }
                if *phase == Phase::Ready {
                    committed = true;
                    return false;
                }
                if activation
                    .as_ref()
                    .is_some_and(|activation| !activation.commit_locked())
                {
                    if let Phase::Staged {
                        streams_bound,
                        streams_abandoned,
                        ..
                    } = phase
                    {
                        *streams_bound = false;
                        *streams_abandoned = true;
                    }
                    return true;
                }
                committed = true;
                *phase = Phase::Ready;
                true
            })();
            // watch wakes receivers synchronously after releasing its phase lock.
            // Release our outer fences before those wakers may reenter discovery.
            drop(guard.take());
            drop(binding.take());
            drop(admission.take());
            changed
        });
        if let Some(activation) = activation {
            if committed {
                activation.complete();
            } else if !self.streams_bound() {
                activation.abort();
            }
        }
        committed
    }

    pub(crate) fn has_committed(&self) -> bool {
        matches!(
            *self.phase.borrow(),
            Phase::Ready | Phase::Closed { committed: true }
        )
    }

    pub(crate) fn close_staging(&self) {
        let mut closed = false;
        self.phase.send_if_modified(|phase| {
            if !matches!(*phase, Phase::Staged { .. }) {
                return false;
            }
            *phase = Phase::Closed { committed: false };
            closed = true;
            true
        });
        if closed {
            let activation = self
                .activation
                .lock()
                .expect("plugin stream binding poisoned")
                .as_ref()
                .and_then(Weak::upgrade);
            if let Some(activation) = activation {
                activation.abort();
            }
        }
    }

    pub(crate) fn close(&self) {
        self.phase.send_if_modified(|phase| {
            if matches!(*phase, Phase::Closed { .. }) {
                return false;
            }
            *phase = Phase::Closed {
                committed: *phase == Phase::Ready,
            };
            true
        });
        if !self.has_committed() {
            let activation = self
                .activation
                .lock()
                .expect("plugin stream binding poisoned")
                .as_ref()
                .and_then(Weak::upgrade);
            if let Some(activation) = activation {
                activation.abort();
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
