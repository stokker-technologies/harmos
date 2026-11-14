/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Conditional Plugin Registry

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
// Third Party Imports
use prost::Message;
use serde::{Deserialize, Serialize};
// Local Imports
use super::{Error, PluginManagerSettings};
use harmos_domain::model::storage::{ObjectCatalog, ObjectDefinition, ObjectRetention, StoredObject};
use harmos_protocol::client as wire;
use harmos_storage::{CompareExchange, ObjectVersion, ObjectVersionToken, StorageManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug)]
struct RegistryObject {
    id: String,
    payload: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Registry {
    pub next_revision: u64,
    pub entries: BTreeMap<String, SavedEntry>,
    pub creates: BTreeMap<String, SavedCreate>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SavedEntry {
    pub entry: Vec<u8>,
    pub executable: Option<String>,
    pub executable_digest: Option<[u8; 32]>,
    pub generation: u64,
    #[serde(default)]
    pub settings: Vec<u8>,
    #[serde(default)]
    pub settings_revision: Vec<u8>,
    #[serde(default)]
    pub settings_receipts: BTreeMap<String, SavedCreate>,
    #[serde(default)]
    pub replacements: BTreeMap<String, SavedCreate>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SavedCreate {
    pub request: Vec<u8>,
    pub response: Vec<u8>,
}

pub(super) struct Store {
    pub(super) storage: StorageManager,
    namespace: String,
    version: Option<ObjectVersionToken>,
    max_bytes: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_domain::Identified for RegistryObject {
    const ID: &'static str = "harmos.plugins.registry";
}
impl StoredObject for RegistryObject {
    type Key = str;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;
    fn key(&self) -> &str {
        &self.id
    }
}

impl Registry {
    pub fn empty() -> Self {
        Self {
            next_revision: 1,
            entries: BTreeMap::new(),
            creates: BTreeMap::new(),
        }
    }
    pub fn revision(&mut self) -> Result<Vec<u8>, Error> {
        let revision = self.next_revision;
        self.next_revision =
            revision.checked_add(1).ok_or_else(|| Error::new("exhausted", "Set Plugin Revision", "create a new manager namespace"))?;
        Ok(revision.to_be_bytes().to_vec())
    }
    pub fn values(&self) -> Result<BTreeMap<String, wire::PluginEntry>, Error> {
        self.entries
            .iter()
            .map(|(name, saved)| {
                let entry = wire::PluginEntry::decode(saved.entry.as_slice()).map_err(|error| problem("Restore Plugin Entry", error))?;
                Ok((name.clone(), entry))
            })
            .collect()
    }
}

impl Store {
    pub async fn open(storage: StorageManager, settings: &PluginManagerSettings) -> Result<(Self, Registry), Error> {
        let saved = storage
            .get_object::<RegistryObject>(&settings.namespace, ObjectVersion::Latest)
            .await
            .map_err(|error| problem("Restore Plugin Registry", error))?;
        let (version, registry) = match saved {
            Some(saved) => {
                let registry = serde_json::from_slice(&saved.value.payload).map_err(|error| problem("Decode Plugin Registry", error))?;
                (Some(saved.version.clone()), registry)
            }
            None => (None, Registry::empty()),
        };
        let store = Self {
            storage,
            namespace: settings.namespace.clone(),
            version,
            max_bytes: settings.registry_bytes.get(),
        };
        Ok((store, registry))
    }
    pub async fn save(&mut self, registry: &Registry) -> Result<(), Error> {
        let payload = serde_json::to_vec(registry).map_err(|error| problem("Encode Plugin Registry", error))?;
        if payload.len() > self.max_bytes {
            return Err(Error::new(
                "registry_capacity",
                "Save Plugin Registry",
                "reduce the entry set or configure a larger registry capacity",
            ));
        }
        let object = RegistryObject { id: self.namespace.clone(), payload };
        let result =
            self.storage.compare_exchange(self.version.clone(), &object).await.map_err(|error| problem("Save Plugin Registry", error))?;
        match result {
            CompareExchange::Applied(receipt) => {
                self.version = Some(receipt.version);
                Ok(())
            }
            CompareExchange::Conflict { .. } => {
                Err(Error::new("conflict", "Save Plugin Registry", "stop the competing manager and reopen this namespace"))
            }
        }
    }
}

/* === Public Functions === */

pub fn catalog() -> ObjectCatalog {
    ObjectCatalog::new().entry(ObjectDefinition::of::<RegistryObject>())
}

/* === Private Functions === */

pub(super) fn problem(action: &str, source: impl std::fmt::Display) -> Error {
    Error::new("manager_failure", format!("{action}: {source}"), "correct the reported cause and retry the action")
}

/* === Tests === */
