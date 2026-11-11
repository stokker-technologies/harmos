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
// Third Party Imports
use semver::Version;
use serde::Deserialize;
// Local Imports
use super::contributions::{self, Section};
use super::{contracts, listeners, observers, publications};
use crate::operations::OperationDefinition;
use crate::{FRAGMENT_LIMIT, ScanError, StreamDeclaration};

/* === Constants === */

const OWNED: [&str; 14] = [
    "capabilities",
    "contracts",
    "contributions",
    "guest",
    "listeners",
    "observers",
    "operations",
    "plugin",
    "publications",
    "requires",
    "restart",
    "schema",
    "sidecar",
    "subscribe",
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub id: String,
    pub version: Version,
    pub description: String,
    pub config: String,
    pub operations: Vec<OperationDefinition>,
    pub contracts: contracts::Manifest,
    pub listeners: Vec<crate::listeners::Definition>,
    pub streams: Vec<StreamDeclaration>,
    pub observers: Vec<observers::Declaration>,
    pub sections: Vec<Section>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facility {
    Sidecar,
    Guest,
}

#[derive(Debug, thiserror::Error)]
pub enum DeclarationError {
    #[error(transparent)]
    Schema(#[from] harmos_domain::schema::graph::Error),

    #[error(transparent)]
    PublicationInventory(#[from] publications::Error),

    #[error(transparent)]
    ContractInventory(#[from] contracts::Error),

    #[error(transparent)]
    ContributionInventory(#[from] contributions::Error),

    #[error(transparent)]
    ListenerInventory(#[from] listeners::Error),

    #[error(transparent)]
    ObserverInventory(#[from] observers::Error),

    #[error("guest observer callbacks are unsupported")]
    UnsupportedObservers,

    #[error("embedded declarations are only supported for guests; native plugins use Describe")]
    UnsupportedFacility,

    #[error("the artifact carries no Harmos {facility} declaration")]
    MissingIdentity { facility: &'static str },

    #[error("the artifact carries more than one Harmos {facility} declaration")]
    DuplicateIdentity { facility: &'static str },

    #[error("a Harmos metadata fragment exceeds its {FRAGMENT_LIMIT}-byte bound")]
    OversizedFragment,

    #[error("a Harmos metadata fragment is invalid: {source}")]
    Malformed {
        #[source]
        source: serde_json::Error,
    },

    #[error("a Harmos metadata fragment declares the unknown kind `{kind}`")]
    UnknownKind { kind: String },

    #[error("`{id}` is not a portable Harmos artifact id")]
    InvalidId { id: String },

    #[error("the Harmos plugin version is invalid: {source}")]
    Version {
        #[source]
        source: semver::Error,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Fragment {
    Plugin(contracts::Identity),
    Contracts(Box<contracts::NamedFragment>),
    Contributions(Box<contributions::Manifest>),
    Listeners(Box<listeners::NamedManifest>),
    Observers(Box<observers::Manifest>),
    Publications(publications::NamedManifest),
    Schema {
        name: String,
        fields: Vec<harmos_domain::schema::declaration::Field<String>>,

        #[serde(default)]
        representation: Option<harmos_domain::schema::wire::Representation>,
    },
}

/* === Traits === */

/* === Implementations === */

impl Facility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sidecar => "sidecar",
            Self::Guest => "guest",
        }
    }
}

impl Declaration {
    pub fn inspect(artifact: &[u8], facility: Facility) -> Result<Self, DeclarationError> {
        Self::inspect_with_settings(artifact, facility, &harmos_domain::config::SchemaSettings::default())
    }

    pub fn inspect_with_settings(
        artifact: &[u8], facility: Facility, settings: &harmos_domain::config::SchemaSettings,
    ) -> Result<Self, DeclarationError> {
        if facility != Facility::Guest {
            return Err(DeclarationError::UnsupportedFacility);
        }
        let name = facility.as_str();
        let mut identity = None;
        let mut schemas = Vec::new();
        let mut contract_manifests = Vec::new();
        let mut contribution_manifests = Vec::new();
        let mut listener_manifests = Vec::new();
        let mut observer_manifests = Vec::new();
        let mut publications = Vec::new();
        let mut settings = *settings;
        for fragment in harmos_operations::artifacts::fragments_with_settings::<Fragment>(artifact, &OWNED, &mut settings)? {
            match fragment {
                Fragment::Plugin(common) => {
                    if common.facility != name {
                        continue;
                    }
                    if identity.is_some() {
                        return Err(DeclarationError::DuplicateIdentity { facility: name });
                    }
                    if !harmos_domain::identity::segment(&common.id) {
                        return Err(DeclarationError::InvalidId { id: common.id });
                    }
                    let version = Version::parse(&common.version).map_err(|source| DeclarationError::Version { source })?;
                    identity = Some((common.id, version, common.description, common.config));
                }
                Fragment::Contracts(manifest) => contract_manifests.push(*manifest),
                Fragment::Contributions(manifest) => contribution_manifests.push(*manifest),
                Fragment::Listeners(manifest) => listener_manifests.push(*manifest),
                Fragment::Observers(manifest) => observer_manifests.push(*manifest),
                Fragment::Publications(manifest) => publications.push(manifest),
                Fragment::Schema { name, fields, representation } => {
                    schemas.push(harmos_domain::schema::declaration::Named { name, fields, representation })
                }
            }
        }
        let Some((id, version, description, config)) = identity else {
            return Err(DeclarationError::MissingIdentity { facility: name });
        };

        // Resolve The Complete Inert Dictionary Before Selecting Owned Sections.
        let listener_manifests = listener_manifests
            .into_iter()
            .map(|manifest| manifest.resolve(&schemas, &mut settings))
            .collect::<Result<_, listeners::Error>>()?;
        let publications = publications
            .into_iter()
            .map(|manifest| manifest.resolve(&schemas, &mut settings))
            .collect::<Result<_, publications::Error>>()?;
        let contracts = contracts::select(contract_manifests, name, &id, &schemas, &mut settings)?;
        let sections = contributions::select(contribution_manifests, name, &id)?;
        let listeners = listeners::select(listener_manifests, name, &id, &contracts.provides)?.unwrap_or_default();
        let operations = contracts.provides.iter().map(|contract| contract.operation().clone()).collect();
        let streams = publications::select_with_settings(publications, name, &id, &settings)?;

        if observer_manifests.iter().any(|manifest| manifest.facility == name && manifest.owner == id) {
            let selected = observers::select_manifest(observer_manifests, name, &id, &streams)?;
            if !selected.observers.is_empty() {
                return Err(DeclarationError::UnsupportedObservers);
            }
        }
        let observers = Vec::new();
        Ok(Self {
            id,
            version,
            description,
            config,
            operations,
            contracts,
            listeners,
            streams,
            observers,
            sections,
        })
    }

    pub fn section(&self, namespace: &str) -> Option<&Section> {
        self.sections.iter().find(|section| section.namespace == namespace)
    }
}

impl From<ScanError> for DeclarationError {
    fn from(error: ScanError) -> Self {
        match error {
            ScanError::Budget { budget, used, allowed } => {
                Self::Schema(harmos_domain::schema::graph::Error::Allowance { budget, used, allowed })
            }
            ScanError::Oversized => Self::OversizedFragment,
            ScanError::Malformed { source } => Self::Malformed { source },
            ScanError::UnknownKind { kind } => Self::UnknownKind { kind },
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
