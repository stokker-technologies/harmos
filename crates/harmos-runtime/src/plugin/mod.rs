/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod host;

mod backend;
mod definition;
mod definitions;
mod events;
mod manager;
mod reload;
mod router;
mod scope;
mod status;

/* === Re-Exports === */

pub use backend::PluginBackend;
pub use definition::{PluginArtifact, PluginContributions, PluginDefinition, PluginDefinitionError, PluginKind};
pub use definitions::PluginDefinitions;
pub use events::{PluginEvent, PluginEventKind, PluginEvents};
// Explicit Items Shadow The Contract's Same-Named Ones: `Error` Is The Host's,
// `Requirement` The Plugin Dependency; Grant Requirements Are
// `catalog::RequirementEntry`.
pub use harmos_domain::{Refusal, Registrar, Registration, RegistrationError, Requirement, StreamDeclaration, inventory};
pub use harmos_sdk::plugin::*;
pub use host::PluginHost;
#[cfg(feature = "plugin-sidecar")]
pub(crate) use host::{Driving, Lease};
pub use manager::{
    PluginCleanup, PluginCleanupFailure, PluginCleanupStatus, PluginFailure, PluginInstance, PluginLifecycle, PluginLifecycleAuthority,
    PluginLifecyclePhase, PluginManager, PluginReloadPlan, PluginTransition,
};
pub use router::{DispatchError, PluginCaller, PluginEndpoint, Router};
pub use scope::PluginScope;
pub use status::{PluginFailureClass, PluginGeneration, PluginState, PluginStatus};

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
// Third Party Imports
use semver::{Version, VersionReq};
// Local Imports
use reload::ReloadImpact;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginGraph {
    ordered: Vec<PluginDefinition>,
    topology: crate::supervision::dependencies::Graph<String>,
    required_providers: BTreeMap<String, BTreeMap<crate::Identifier, String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationProvider {
    pub consumer: String,
    pub operation: crate::Identifier,
    pub provider: String,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("plugin `{artifact}` has an invalid definition: {source}")]
    Definition {
        artifact: String,

        #[source]
        source: Box<PluginDefinitionError>,
    },

    #[error("plugin `{artifact}` requires incompatible wire operation `{operation}` from `{provider}`: {reason}")]
    IncompatibleOperation { artifact: String, operation: String, provider: String, reason: &'static str },

    #[error("plugin `{artifact}` has multiple compatible providers for operation `{operation}`")]
    AmbiguousOperation { artifact: String, operation: String },

    #[error("provider selection for `{artifact}` operation `{operation}` is invalid: {reason}")]
    ProviderSelection { artifact: String, operation: String, reason: &'static str },

    #[error("stream `{id}` is declared by both `{first}` and `{second}`")]
    StreamConflict { id: String, first: String, second: String },

    #[error("artifact `{id}` is registered more than once")]
    Duplicate { id: String },

    #[error("cannot reload unknown artifact `{id}`")]
    Unknown { id: String },

    #[error("artifact `{artifact}` requires missing artifact `{dependency}`")]
    Missing { artifact: String, dependency: String },

    #[error("artifact `{artifact}` requires `{dependency}` {required}, but {found} is registered")]
    Incompatible { artifact: String, dependency: String, required: VersionReq, found: Version },

    #[error("artifact dependency cycle reaches `{artifact}`")]
    Cycle { artifact: String },
}

/* === Traits === */

/* === Implementations === */

impl PluginGraph {
    pub fn resolve(artifacts: impl IntoIterator<Item = PluginDefinition>) -> Result<Self, Error> {
        Self::resolve_with_providers(artifacts, [])
    }

    pub fn resolve_with_providers(
        artifacts: impl IntoIterator<Item = PluginDefinition>, selections: impl IntoIterator<Item = OperationProvider>,
    ) -> Result<Self, Error> {
        let mut choices = BTreeMap::new();
        for selection in selections {
            let key = (selection.consumer.clone(), selection.operation.clone());
            let reason = if choices.len() >= 4096 {
                Some("selection count exceeds 4096")
            } else if choices.contains_key(&key) {
                Some("requirement is selected more than once")
            } else {
                None
            };
            if let Some(reason) = reason {
                return Err(Error::ProviderSelection { artifact: selection.consumer, operation: selection.operation.to_string(), reason });
            }
            choices.insert(key, selection.provider);
        }
        let mut declared = BTreeMap::<String, PluginDefinition>::new();
        let mut streams = BTreeMap::new();
        let mut operations = BTreeMap::<crate::Identifier, Vec<String>>::new();
        for mut artifact in artifacts {
            let id = artifact.id.clone();
            if declared.contains_key(&id) {
                return Err(Error::Duplicate { id });
            }
            artifact.validate().map_err(|source| Error::Definition { artifact: id.clone(), source: Box::new(source) })?;
            for operation in &artifact.contributions.operations {
                let providers = operations.entry(operation.id.clone()).or_default();
                providers.push(id.clone());
            }
            for stream in &artifact.contributions.streams {
                if let Some(first) = streams.insert(stream.id.clone(), id.clone()) {
                    return Err(Error::StreamConflict { id: stream.id.clone(), first, second: id });
                }
            }
            declared.insert(id, artifact);
        }

        let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        let mut required_providers = BTreeMap::new();
        for artifact in declared.values() {
            let mut edges = BTreeSet::new();
            for requirement in &artifact.requires {
                let dependency = declared
                    .get(&requirement.id)
                    .ok_or_else(|| Error::Missing { artifact: artifact.id.clone(), dependency: requirement.id.clone() })?;
                if !requirement.version.matches(&dependency.version) {
                    return Err(Error::Incompatible {
                        artifact: artifact.id.clone(),
                        dependency: requirement.id.clone(),
                        required: requirement.version.clone(),
                        found: dependency.version.clone(),
                    });
                }
                edges.insert(requirement.id.clone());
            }
            let mut selected = BTreeMap::new();
            if let Some(required) = &artifact.contributions.required_contracts {
                for required in required {
                    let contract = &required.contract;
                    let operation = contract.operation();
                    let choice = choices.remove(&(artifact.id.clone(), operation.id.clone()));
                    // No Plugin Provides It: The Host Routes It To Its Own Provider
                    let Some(providers) = operations.get(&operation.id) else {
                        if choice.is_some() {
                            return Err(Error::ProviderSelection {
                                artifact: artifact.id.clone(),
                                operation: operation.id.to_string(),
                                reason: "selected instance does not provide the required operation",
                            });
                        }
                        continue;
                    };
                    let provider = if let Some(choice) = choice {
                        providers.iter().find(|provider| **provider == choice).ok_or_else(|| Error::ProviderSelection {
                            artifact: artifact.id.clone(),
                            operation: operation.id.to_string(),
                            reason: "selected instance does not provide the required operation",
                        })?
                    } else {
                        let mut compatible = providers.iter().filter(|provider| {
                            *provider != &artifact.id
                                && operation.visibility == crate::operations::Visibility::Shared
                                && declared[*provider].contributions.wire_contracts.iter().any(|actual| actual == contract)
                        });
                        let first = compatible.next();
                        if compatible.next().is_some() {
                            return Err(Error::AmbiguousOperation { artifact: artifact.id.clone(), operation: operation.id.to_string() });
                        }
                        first.unwrap_or(&providers[0])
                    };
                    let actual =
                        declared[provider].contributions.wire_contracts.iter().find(|actual| actual.operation().id == operation.id);
                    let reason = if actual != Some(contract) {
                        Some("version, schema, policy, visibility or replay differs")
                    } else if operation.visibility == crate::operations::Visibility::Host
                        || provider == &artifact.id
                        || provider != &artifact.id && operation.visibility != crate::operations::Visibility::Shared
                    {
                        Some("operation visibility denies this plugin caller")
                    } else {
                        None
                    };
                    if let Some(reason) = reason {
                        return Err(Error::IncompatibleOperation {
                            artifact: artifact.id.clone(),
                            operation: operation.id.to_string(),
                            provider: provider.clone(),
                            reason,
                        });
                    }
                    selected.insert(operation.id.clone(), provider.clone());
                    if provider != &artifact.id {
                        edges.insert(provider.clone());
                    }
                }
            }
            dependencies.insert(artifact.id.clone(), edges);
            required_providers.insert(artifact.id.clone(), selected);
        }

        if let Some(((artifact, operation), _)) = choices.into_iter().next() {
            return Err(Error::ProviderSelection {
                artifact,
                operation: operation.to_string(),
                reason: "selection has no matching consumer requirement",
            });
        }

        let topology = crate::supervision::dependencies::Graph::new(dependencies).map_err(|error| match error {
            crate::supervision::dependencies::Invalid::Missing { owner, dependency } => Error::Missing { artifact: owner, dependency },
            crate::supervision::dependencies::Invalid::Cycle { owner } => Error::Cycle { artifact: owner },
        })?;
        let ordered =
            topology.ordered().iter().map(|id| declared.get(id).expect("every graph node came from the declaration map").clone()).collect();
        Ok(Self { ordered, topology, required_providers })
    }

    pub fn ordered(&self) -> &[PluginDefinition] {
        &self.ordered
    }

    pub(super) fn knows(&self, owner: &str) -> bool {
        self.ordered.iter().any(|definition| definition.id == owner)
    }

    pub(super) fn same_contract(&self, owner: &str, other: &Self) -> bool {
        let before = self.ordered.iter().find(|definition| definition.id == owner);
        let after = other.ordered.iter().find(|definition| definition.id == owner);
        before.is_some() && before == after && self.required_providers.get(owner) == other.required_providers.get(owner)
    }

    pub(super) fn dependencies(&self, owner: &str) -> &BTreeSet<String> {
        self.topology.dependencies(owner).expect("registered plugin dependency owner")
    }

    pub(super) fn required_providers(&self, owner: &str) -> &BTreeMap<crate::Identifier, String> {
        &self.required_providers[owner]
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
