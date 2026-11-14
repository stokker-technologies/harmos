/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Plugin Entries

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::host::PluginDescription;
use harmos_protocol::host::PluginEntry;
use harmos_protocol::host::PluginGuest;
use harmos_protocol::host::PluginPlacement;
use harmos_protocol::host::PluginState;
use harmos_protocol::host::PluginValue;

use super::{
    Error,
    actor::{Actor, changed, is_guest, unavailable},
    persistence::{SavedEntry, problem},
};
use harmos_protocol::host as wire;
use prost::Message;
use std::collections::BTreeMap;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Actor {
    pub(super) async fn guest(&mut self, description: PluginDescription) -> Result<PluginEntry, Error> {
        let name = description.id.clone();
        let mut registry = self.registry.clone();
        let mut entry = if let Some(saved) = registry.entries.get(&name) {
            let entry = PluginEntry::decode(saved.entry.as_slice()).map_err(|error| problem("Register Guest Plugin", error))?;
            if !is_guest(&entry) {
                return Err(Error::new("entry_exists", "Register Guest Plugin", "choose an unused plugin identity"));
            }
            entry
        } else {
            if registry.entries.len() >= self.settings.entries.get() {
                return Err(Error::new("manager_capacity", "Register Guest Plugin", "configure capacity for plugin entries"));
            }
            PluginEntry {
                name: name.clone(),
                placement: Some(PluginPlacement {
                    kind: Some(wire::plugin_placement::Kind::Guest(PluginGuest {})),
                }),
                value: Some(PluginValue { enabled: true }),
                revision: registry.revision()?,
                ..Default::default()
            }
        };
        entry.declarations_digest = description.declarations.as_ref().expect("inspected guest declarations").digest.clone();
        entry.description = Some(description);
        entry.state = if entry.value.is_some_and(|value| value.enabled) { PluginState::Connecting } else { PluginState::Stopped } as i32;
        if let Some(saved) = registry.entries.get_mut(&name) {
            saved.entry = entry.encode_to_vec();
        } else {
            registry.entries.insert(
                name,
                SavedEntry {
                    entry: entry.encode_to_vec(),
                    executable: None,
                    executable_digest: None,
                    generation: 0,
                    settings: Vec::new(),
                    settings_revision: Vec::new(),
                    settings_receipts: BTreeMap::new(),
                    replacements: BTreeMap::new(),
                },
            );
        }
        self.commit(registry, changed(entry.clone())).await?;
        Ok(entry)
    }

    pub(super) async fn guest_health(
        &mut self, name: String, state: PluginState, activation: u64, failure: Option<String>,
    ) -> Result<(), Error> {
        let Some(saved) = self.registry.entries.get(&name) else {
            return Ok(());
        };
        let mut entry = PluginEntry::decode(saved.entry.as_slice()).map_err(|error| problem("Observe Guest Plugin", error))?;
        if !is_guest(&entry) || !entry.value.is_some_and(|value| value.enabled) {
            return Ok(());
        }
        let previous = entry.clone();
        entry.state = state as i32;
        entry.activation = activation;
        entry.failure = failure.map(|cause| unavailable(&name, cause));
        if entry == previous {
            return Ok(());
        }
        let mut registry = self.registry.clone();
        registry.entries.get_mut(&name).expect("observed entry exists").entry = entry.encode_to_vec();
        self.commit(registry, changed(entry)).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
