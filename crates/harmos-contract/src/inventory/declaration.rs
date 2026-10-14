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
use std::time::Duration;
// Third Party Imports
use semver::Version;
use serde::Deserialize;
// Local Imports
use super::contributions::{self, Section};
use super::{contracts, listeners, observers, publications};
use crate::operations::{Contract, Definition};
use crate::{FRAGMENT_LIMIT, ScanError, StreamDeclaration, artifact_subset};

/* === Constants === */
const OWNED: [&str; 17] = [
    "capabilities",
    "contracts",
    "contributions",
    "egress",
    "guest",
    "invoke",
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
    "survey",
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
    pub operations: Vec<Definition>,
    pub contracts: contracts::Manifest,
    pub listeners: Vec<crate::listeners::Definition>,
    pub streams: Vec<StreamDeclaration>,
    pub observers: Vec<observers::Declaration>,
    pub sections: Vec<Section>,
    pub carrier: CarrierSection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct RestartPolicy {
    pub credit: u32,
    pub sustained_health_ms: u64,
    pub backoff_base_ms: u64,
    pub backoff_cap_ms: u64,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facility {
    Sidecar,
    Guest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CarrierSection {
    Sidecar {
        restart: Option<RestartPolicy>,
        observer_control: Option<Box<Contract>>,
    },
    Guest,
}

#[derive(Debug, thiserror::Error)]
pub enum DeclarationError {
    #[error("the artifact uses retired Harmos metadata; rebuild it with the current SDK")]
    IncompatiblePayload,
    #[error(transparent)]
    Schema(#[from] crate::schema::graph::Error),
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
    #[error("the Harmos sidecar restart policy is invalid")]
    InvalidRestartPolicy,
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
    Contracts(Box<contracts::NamedManifest>),
    Contributions(Box<contributions::Manifest>),
    Listeners(Box<listeners::NamedManifest>),
    Observers(Box<observers::NamedManifest>),
    Restart {
        facility: String,
        owner: String,
        policy: RestartPolicy,
    },
    Publications(publications::NamedManifest),
    Schema {
        name: String,
        fields: Vec<crate::schema::declaration::Field<String>>,
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
    #[tracing::instrument(level = "debug", skip_all, fields(artifact_bytes = artifact.len()))]
    pub fn inspect(artifact: &[u8], facility: Facility) -> Result<Self, DeclarationError> {
        let name = facility.as_str();
        let mut identity = None;
        let mut schemas = Vec::new();
        let mut contract_manifests = Vec::new();
        let mut contribution_manifests = Vec::new();
        let mut listener_manifests = Vec::new();
        let mut observer_manifests = Vec::new();
        let mut publications = Vec::new();
        let mut restart_policies = Vec::new();
        let mut settings = crate::config::SchemaSettings::default();
        for fragment in
            crate::artifacts::fragments_with_settings::<Fragment>(artifact, &OWNED, &mut settings)?
        {
            match fragment {
                Fragment::Plugin(common) => {
                    if common.facility != name {
                        continue;
                    }
                    if identity.is_some() {
                        return Err(DeclarationError::DuplicateIdentity { facility: name });
                    }
                    if !artifact_subset(&common.id) {
                        return Err(DeclarationError::InvalidId { id: common.id });
                    }
                    let version = Version::parse(&common.version)
                        .map_err(|source| DeclarationError::Version { source })?;
                    identity = Some((common.id, version, common.description, common.config));
                }
                Fragment::Contracts(manifest) => contract_manifests.push(*manifest),
                Fragment::Contributions(manifest) => contribution_manifests.push(*manifest),
                Fragment::Listeners(manifest) => listener_manifests.push(*manifest),
                Fragment::Observers(manifest) => observer_manifests.push(*manifest),
                Fragment::Restart {
                    facility,
                    owner,
                    policy,
                } => restart_policies.push((facility, owner, policy)),
                Fragment::Publications(manifest) => publications.push(manifest),
                Fragment::Schema { name, fields } => {
                    schemas.push(crate::schema::declaration::Named { name, fields })
                }
            }
        }
        let Some((id, version, description, config)) = identity else {
            return Err(DeclarationError::MissingIdentity { facility: name });
        };

        // Resolve The Complete Inert Dictionary Before Selecting Owned Sections.
        let contract_manifests = contract_manifests
            .into_iter()
            .map(|manifest| manifest.resolve(&schemas, &mut settings))
            .collect::<Result<_, contracts::Error>>()?;
        let listener_manifests = listener_manifests
            .into_iter()
            .map(|manifest| manifest.resolve(&schemas, &mut settings))
            .collect::<Result<_, listeners::Error>>()?;
        let observer_manifests = observer_manifests
            .into_iter()
            .map(|manifest| manifest.resolve(&schemas, &mut settings))
            .collect::<Result<_, observers::Error>>()?;
        let publications = publications
            .into_iter()
            .map(|manifest| manifest.resolve(&schemas, &mut settings))
            .collect::<Result<_, publications::Error>>()?;
        let contracts = contracts::select(contract_manifests, name, &id)?;
        let sections = contributions::select(contribution_manifests, name, &id)?;
        let listeners = listeners::select(listener_manifests, name, &id, &contracts.provides)?
            .unwrap_or_default();
        let operations = contracts
            .provides
            .iter()
            .map(|contract| contract.operation().clone())
            .collect();
        let streams = publications::select(publications, name, &id)?;

        // Select The CarrierSection Section
        let (observers, carrier) = match facility {
            Facility::Sidecar => {
                let restart = restart(restart_policies, &id)?;
                let selected = observers::select_manifest(observer_manifests, name, &id, &streams)?;
                observers::validate_control(
                    &selected.observers,
                    selected.observer_control.as_ref(),
                    &contracts.provides,
                )?;
                let carrier = CarrierSection::Sidecar {
                    restart,
                    observer_control: selected.observer_control.map(Box::new),
                };
                (selected.observers, carrier)
            }
            Facility::Guest => {
                if observer_manifests
                    .iter()
                    .any(|manifest| manifest.facility == name && manifest.owner == id)
                {
                    let selected =
                        observers::select_manifest(observer_manifests, name, &id, &streams)?;
                    if !selected.observers.is_empty() || selected.observer_control.is_some() {
                        return Err(DeclarationError::UnsupportedObservers);
                    }
                }
                (Vec::new(), CarrierSection::Guest)
            }
        };
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
            carrier,
        })
    }

    pub fn section(&self, namespace: &str) -> Option<&Section> {
        self.sections
            .iter()
            .find(|section| section.namespace == namespace)
    }

    pub fn restart(&self) -> Option<RestartPolicy> {
        match &self.carrier {
            CarrierSection::Sidecar { restart, .. } => *restart,
            CarrierSection::Guest => None,
        }
    }

    pub fn observer_control(&self) -> Option<&Contract> {
        match &self.carrier {
            CarrierSection::Sidecar {
                observer_control, ..
            } => observer_control.as_deref(),
            CarrierSection::Guest => None,
        }
    }
}

impl RestartPolicy {
    pub fn enabled(self) -> bool {
        self.credit > 0
    }

    pub fn bounded(self) -> Self {
        Self {
            credit: self.credit.min(10),
            sustained_health_ms: self.sustained_health_ms.min(60 * 60 * 1_000),
            backoff_base_ms: self.backoff_base_ms.min(60 * 1_000),
            backoff_cap_ms: self.backoff_cap_ms.min(5 * 60 * 1_000),
        }
    }

    fn validate(self) -> Result<(), DeclarationError> {
        if self.sustained_health_ms == 0
            || self.backoff_base_ms == 0
            || self.backoff_cap_ms < self.backoff_base_ms
        {
            return Err(DeclarationError::InvalidRestartPolicy);
        }
        Ok(())
    }

    pub fn sustained_health(self) -> Duration {
        Duration::from_millis(self.bounded().sustained_health_ms)
    }

    pub fn backoff_base(self) -> Duration {
        Duration::from_millis(self.bounded().backoff_base_ms)
    }

    pub fn backoff_cap(self) -> Duration {
        Duration::from_millis(self.bounded().backoff_cap_ms)
    }
}

impl From<ScanError> for DeclarationError {
    fn from(error: ScanError) -> Self {
        match error {
            ScanError::IncompatiblePayload => Self::IncompatiblePayload,
            ScanError::Budget => Self::Schema(crate::schema::graph::Error::Budget),
            ScanError::Oversized => Self::OversizedFragment,
            ScanError::Malformed { source } => Self::Malformed { source },
            ScanError::UnknownKind { kind } => Self::UnknownKind { kind },
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn restart(
    policies: Vec<(String, String, RestartPolicy)>,
    owner: &str,
) -> Result<Option<RestartPolicy>, DeclarationError> {
    let mut selected = policies
        .into_iter()
        .filter(|(facility, candidate, _)| facility == "sidecar" && candidate == owner);
    let Some((_, _, policy)) = selected.next() else {
        return Ok(None);
    };
    if selected.next().is_some() {
        return Err(DeclarationError::InvalidRestartPolicy);
    }
    policy.validate()?;
    Ok(Some(policy))
}

/* === Tests === */
