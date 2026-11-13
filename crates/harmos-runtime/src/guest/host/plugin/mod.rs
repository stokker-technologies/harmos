/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod backend;
mod definition;
mod definitions;
mod events;
mod host;
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
pub use harmos_operations::plugin::*;
pub use harmos_operations::{Refusal, Requirement, StreamDeclaration};
pub use host::PluginHost;

pub use manager::{
    PluginCleanup, PluginCleanupFailure, PluginCleanupStatus, PluginFailure, PluginInstance, PluginLifecycle, PluginLifecycleAuthority,
    PluginLifecyclePhase, PluginManager, PluginTransition,
};
pub use router::{DispatchError, PluginCaller, PluginEndpoint, Router};
pub use scope::PluginScope;
pub use status::{PluginFailureClass, PluginGeneration, PluginState, PluginStatus};

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;

// Third Party Imports

// Local Imports
use reload::ReloadImpact;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestInventory {
    ordered: Vec<PluginDefinition>,
    required_providers: BTreeMap<String, BTreeMap<crate::Identifier, String>>,
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

    #[error("stream `{id}` is declared by both `{first}` and `{second}`")]
    StreamConflict { id: String, first: String, second: String },

    #[error("artifact `{id}` is registered more than once")]
    Duplicate { id: String },

    #[error("cannot reload unknown artifact `{id}`")]
    Unknown { id: String },
}

/* === Traits === */

/* === Implementations === */

impl GuestInventory {
    pub fn resolve(artifacts: impl IntoIterator<Item = PluginDefinition>) -> Result<Self, Error> {
        let mut declared = BTreeMap::<String, PluginDefinition>::new();
        let mut order = Vec::new();
        let mut streams = BTreeMap::new();
        let mut operations = BTreeMap::<crate::Identifier, Vec<String>>::new();
        for mut artifact in artifacts {
            let id = artifact.id.clone();
            if declared.contains_key(&id) {
                return Err(Error::Duplicate { id });
            }
            artifact.validate().map_err(|source| Error::Definition {
                artifact: id.clone(),
                source: Box::new(source),
            })?;
            for operation in &artifact.contributions.operations {
                let providers = operations.entry(operation.id.clone()).or_default();
                providers.push(id.clone());
            }
            for stream in &artifact.contributions.streams {
                if let Some(first) = streams.insert(stream.id.clone(), id.clone()) {
                    return Err(Error::StreamConflict { id: stream.id.clone(), first, second: id });
                }
            }
            order.push(id.clone());
            declared.insert(id, artifact);
        }

        let mut required_providers = BTreeMap::new();
        for artifact in declared.values() {
            let mut selected = BTreeMap::new();
            if let Some(required) = &artifact.contributions.required_contracts {
                for required in required {
                    let contract = &required.contract;
                    let operation = contract.operation();
                    // No Plugin Provides It: The Host Routes It To Its Own Provider
                    let Some(providers) = operations.get(&operation.id) else {
                        continue;
                    };
                    let provider = {
                        let mut compatible = providers.iter().filter(|provider| {
                            *provider != &artifact.id
                                && operation.visibility == crate::operations::Visibility::Public
                                && declared[*provider].contributions.wire_contracts.iter().any(|actual| actual == contract)
                        });
                        let first = compatible.next();
                        if compatible.next().is_some() {
                            return Err(Error::AmbiguousOperation {
                                artifact: artifact.id.clone(),
                                operation: operation.id.to_string(),
                            });
                        }
                        first.unwrap_or(&providers[0])
                    };
                    let actual =
                        declared[provider].contributions.wire_contracts.iter().find(|actual| actual.operation().id == operation.id);
                    let reason = if actual != Some(contract) {
                        Some("version, schema, policy, visibility or replay differs")
                    } else if provider != &artifact.id && operation.visibility != crate::operations::Visibility::Public {
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
                }
            }
            required_providers.insert(artifact.id.clone(), selected);
        }

        let ordered = order.into_iter().map(|id| declared.remove(&id).expect("validated guest definition")).collect();
        Ok(Self { ordered, required_providers })
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

    pub(super) fn required_providers(&self, owner: &str) -> &BTreeMap<crate::Identifier, String> {
        &self.required_providers[owner]
    }
}

impl Error {
    // The one plugin this error refuses, or `None` when several (a cycle) or none are at fault.
    pub fn artifact(&self) -> Option<&str> {
        match self {
            Self::Definition { artifact, .. }
            | Self::IncompatibleOperation { artifact, .. }
            | Self::AmbiguousOperation { artifact, .. } => Some(artifact),
            Self::StreamConflict { second, .. } => Some(second),
            Self::Duplicate { id } => Some(id),
            Self::Unknown { .. } => None,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
