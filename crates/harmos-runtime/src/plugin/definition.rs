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
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
// Third Party Imports
use semver::Version;
// Local Imports
use super::Requirement;
use harmos_domain::inventory::{StreamDeclaration, operations, publications};
use harmos_domain::{ConfigError, Registration, RegistrationError, artifact_subset};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, PartialEq, Eq)]
pub struct PluginDefinition {
    pub id: String,
    pub version: Version,
    pub artifact: PluginArtifact,
    pub requires: Vec<Requirement>,
    pub configuration: Vec<u8>,
    pub contributions: PluginContributions,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct PluginContributions {
    pub lifecycle: Option<[harmos_operations::lifecycle::Descriptor; 2]>,
    pub operations: Vec<harmos_domain::operations::OperationDefinition>,
    pub wire_contracts: Vec<harmos_domain::operations::Contract>,
    pub required_contracts: Option<Vec<harmos_domain::inventory::contracts::Required>>,
    pub observers: Vec<harmos_domain::inventory::observers::Declaration>,
    pub streams: Vec<StreamDeclaration>,
    pub listeners: Vec<harmos_domain::listeners::Definition>,
    pub registrations: Vec<Registration>,
    pub sections: Vec<harmos_domain::inventory::Section>,
}

/* === Enums === */

#[derive(Clone, PartialEq, Eq)]
pub enum PluginArtifact {
    Sidecar {
        path: PathBuf,
        sha256: [u8; 32],
        arguments: Vec<OsString>,
        directory: Option<PathBuf>,
        environment: BTreeMap<OsString, OsString>,
    },
    LocalSidecar {
        revision: String,
    },
    Embedded {
        revision: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginKind {
    Sidecar,
    Embedded,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginDefinitionError {
    #[error("invalid plugin {field}: {reason}")]
    Invalid { field: &'static str, reason: &'static str },

    #[error(transparent)]
    Configuration(#[from] ConfigError),

    #[error(transparent)]
    Publication(#[from] publications::Error),

    #[error(transparent)]
    Operations(#[from] operations::Error),

    #[error(transparent)]
    Listeners(#[from] harmos_domain::inventory::listeners::Error),

    #[error(transparent)]
    Registration(#[from] RegistrationError),

    #[error("registration `{namespace}` has an invalid JSON payload: {source}")]
    RegistrationPayload {
        namespace: String,

        #[source]
        source: ConfigError,
    },
}

/* === Traits === */

/* === Implementations === */

impl fmt::Debug for PluginDefinition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PluginDefinition")
            .field("id", &self.id)
            .field("version", &self.version)
            .field("artifact", &self.artifact)
            .field("requires", &self.requires)
            .field("configuration", &format_args!("<redacted; {} bytes>", self.configuration.len()))
            .field("contributions", &self.contributions)
            .finish()
    }
}

impl fmt::Debug for PluginContributions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let registrations: Vec<_> = self
            .registrations
            .iter()
            .map(|registration| (&registration.namespace, registration.schema, registration.payload.len()))
            .collect();
        formatter
            .debug_struct("PluginContributions")
            .field("lifecycle", &self.lifecycle)
            .field("operations", &self.operations)
            .field("wire_contracts", &self.wire_contracts)
            .field("required_contracts", &self.required_contracts)
            .field("sections", &self.sections)
            .field("observers", &self.observers)
            .field("streams", &self.streams)
            .field("listeners", &self.listeners)
            .field("registrations", &registrations)
            .finish()
    }
}

impl fmt::Debug for PluginArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sidecar { path, sha256, directory, .. } => formatter
                .debug_struct("Sidecar")
                .field("path", path)
                .field("sha256", sha256)
                .field("arguments", &"<redacted>")
                .field("directory", directory)
                .field("environment", &"<redacted>")
                .finish(),
            Self::LocalSidecar { revision } => formatter.debug_struct("LocalSidecar").field("revision", revision).finish(),
            Self::Embedded { revision } => formatter.debug_struct("Embedded").field("revision", revision).finish(),
        }
    }
}

impl PluginArtifact {
    pub fn kind(&self) -> PluginKind {
        match self {
            Self::Sidecar { .. } | Self::LocalSidecar { .. } => PluginKind::Sidecar,
            Self::Embedded { .. } => PluginKind::Embedded,
        }
    }

    fn validate(&self) -> Result<(), PluginDefinitionError> {
        match self {
            Self::Embedded { revision } | Self::LocalSidecar { revision } => {
                if revision.trim().is_empty() {
                    return Err(invalid("artifact revision", "revision is empty"));
                }
                Ok(())
            }
            Self::Sidecar {
                path, arguments, directory, environment, ..
            } => {
                if !path.is_absolute() || path.as_os_str().as_encoded_bytes().contains(&0) {
                    return Err(invalid("artifact path", "path must be absolute and contain no NUL"));
                }
                if directory.as_ref().is_some_and(|path| !path.is_absolute() || path.as_os_str().as_encoded_bytes().contains(&0)) {
                    return Err(invalid("artifact directory", "directory must be absolute and contain no NUL"));
                }
                if arguments.iter().any(|arg| arg.as_encoded_bytes().contains(&0)) {
                    return Err(invalid("artifact arguments", "argument contains NUL"));
                }
                for (name, value) in environment {
                    let name = name.as_encoded_bytes();
                    if name.is_empty() || name.contains(&0) || name.contains(&b'=') || value.as_encoded_bytes().contains(&0) {
                        return Err(invalid("artifact environment", "invalid variable name or value"));
                    }
                }
                Ok(())
            }
        }
    }
}

impl PluginDefinition {
    #[cfg(feature = "plugin-sidecar")]
    pub fn sidecar(executable: &harmos_sidecar::Executable, configuration: Vec<u8>) -> Result<Self, PluginDefinitionError> {
        let artifact = PluginArtifact::Sidecar {
            path: executable.path().to_path_buf(),
            sha256: executable.sha256(),
            arguments: executable.arguments().to_vec(),
            directory: executable.working_directory().map(std::path::Path::to_path_buf),
            environment: executable.environment().iter().cloned().collect(),
        };
        Self::inspected(executable.declaration(), artifact, configuration)
    }

    #[cfg(feature = "plugin-sidecar")]
    fn inspected(
        declaration: &harmos_domain::inventory::Declaration, artifact: PluginArtifact, configuration: Vec<u8>,
    ) -> Result<Self, PluginDefinitionError> {
        let mut definition = Self {
            id: declaration.id.clone(),
            version: declaration.version.clone(),
            artifact,
            requires: Vec::new(),
            configuration,
            contributions: PluginContributions {
                lifecycle: Some(declaration.contracts.lifecycle.clone()),
                operations: declaration.operations.clone(),
                wire_contracts: declaration.contracts.provides.clone(),
                required_contracts: Some(declaration.contracts.requires.clone()),
                observers: declaration.observers.clone(),
                streams: declaration.streams.clone(),
                listeners: declaration.listeners.clone(),
                sections: declaration.sections.clone(),
                registrations: Vec::new(),
            },
        };
        definition.validate()?;
        Ok(definition)
    }

    pub fn kind(&self) -> PluginKind {
        self.artifact.kind()
    }

    pub(super) fn validate(&mut self) -> Result<(), PluginDefinitionError> {
        if let Some(pair) = &self.contributions.lifecycle {
            use harmos_operations::lifecycle::Action;
            if pair[0].action() != Action::Initialize || pair[1].action() != Action::Shutdown {
                return Err(invalid("SDK lifecycle", "expected ordered initialize/shutdown descriptors"));
            }
        }

        if !harmos_domain::identity::segment(&self.id) {
            return Err(invalid("identity", "identity is outside the portable subset"));
        }
        self.artifact.validate()?;
        harmos_domain::validate_config(&self.configuration)?;

        operations::validate(&self.contributions.operations)?;
        self.contributions.operations.sort_by(|a, b| a.id.cmp(&b.id));

        if self.contributions.wire_contracts.len() > self.contributions.operations.len() {
            return Err(invalid("wire contracts", "wire inventory exceeds operation inventory"));
        }
        let mut wire_ids = BTreeSet::new();
        for contract in &self.contributions.wire_contracts {
            if !wire_ids.insert(&contract.operation().id) || !self.contributions.operations.contains(contract.operation()) {
                return Err(invalid("wire contracts", "contract is duplicated or differs from its operation"));
            }
        }
        self.contributions.wire_contracts.sort_by(|a, b| a.operation().id.cmp(&b.operation().id));
        if let Some(required) = &mut self.contributions.required_contracts {
            if required.len() > 4096 {
                return Err(invalid("required contracts", "required inventory exceeds its limit"));
            }
            required.sort_by(|a, b| a.contract.operation().id.cmp(&b.contract.operation().id));
            if required.windows(2).any(|pair| pair[0].contract.operation().id == pair[1].contract.operation().id) {
                return Err(invalid("required contracts", "required operation is duplicated"));
            }
        }
        let mut sections = BTreeSet::new();
        for section in &self.contributions.sections {
            if !artifact_subset(&section.namespace) || !sections.insert(&section.namespace) {
                return Err(invalid("contribution sections", "section namespace is invalid or repeated"));
            }
        }

        harmos_domain::inventory::observers::validate(&self.contributions.observers, &self.contributions.streams)
            .map_err(|_| invalid("observers", "invalid observer inventory or publication grant"))?;

        if self.contributions.listeners.len() > 4096 {
            return Err(harmos_domain::inventory::listeners::Error::Inventory.into());
        }
        let mut listeners = BTreeSet::new();
        let mut callbacks = BTreeSet::new();
        for listener in &self.contributions.listeners {
            let callback = listener.callback().operation();
            if !listeners.insert(listener.id()) || !callbacks.insert(&callback.id) {
                return Err(harmos_domain::inventory::listeners::Error::Entry { id: listener.id().to_string() }.into());
            }
            if !self.contributions.operations.contains(callback) {
                return Err(harmos_domain::inventory::listeners::Error::Callback { id: callback.id.to_string() }.into());
            }
        }

        let mut streams = BTreeSet::new();
        for stream in &self.contributions.streams {
            publications::validate(stream)?;
            if !streams.insert(&stream.id) {
                return Err(publications::Error::DuplicateStream { id: stream.id.clone() }.into());
            }
        }
        let mut namespaces = BTreeSet::new();
        for registration in &self.contributions.registrations {
            if !artifact_subset(&registration.namespace) {
                return Err(RegistrationError::InvalidNamespace {
                    namespace: registration.namespace.clone(),
                }
                .into());
            }
            if registration.schema == 0 {
                return Err(RegistrationError::ZeroSchema {
                    namespace: registration.namespace.clone(),
                }
                .into());
            }
            if !namespaces.insert(&registration.namespace) {
                return Err(RegistrationError::DuplicateNamespace {
                    namespace: registration.namespace.clone(),
                }
                .into());
            }
            harmos_domain::validate_config(&registration.payload).map_err(|source| PluginDefinitionError::RegistrationPayload {
                namespace: registration.namespace.clone(),
                source,
            })?;
        }

        self.contributions.streams.sort_by(|a, b| a.id.cmp(&b.id));
        self.contributions.registrations.sort_by(|a, b| a.namespace.cmp(&b.namespace));
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(field: &'static str, reason: &'static str) -> PluginDefinitionError {
    PluginDefinitionError::Invalid { field, reason }
}

/* === Tests === */
