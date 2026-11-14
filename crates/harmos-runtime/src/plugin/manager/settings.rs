/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Durable Plugin Settings

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{
    Error,
    actor::{Actor, changed},
    persistence::{Registry, SavedMutation, problem},
};
use harmos_protocol::host::PluginEntry;
use harmos_protocol::host::plugins as wire;
use harmos_protocol::model::Declarations;
use harmos_protocol::model::SettingDeclaration;
use harmos_protocol::model::SettingValue;
use harmos_protocol::model::SettingValues;
use harmos_protocol::model::SettingsDeclarations;
use harmos_protocol::model::setting_value::Value;
use prost::Message;
use std::collections::{BTreeMap, BTreeSet};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub(super) struct Snapshot {
    declarations: Vec<SettingDeclaration>,
    value: SettingValues,
    revision: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Actor {
    pub(super) fn settings_list(&mut self, request: wire::SettingsListRequest) -> Result<wire::SettingsListResponse, Error> {
        let name = request
            .name
            .as_ref()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| Error::new("name_required", "List Plugin Settings", "select a plugin entry"))?;
        if !self.registry.entries.contains_key(name) {
            return Err(Error::new("entry_missing", "List Plugin Settings", "select an existing plugin entry"));
        }
        let snapshot = self.snapshot(request.at.as_ref())?.clone();
        let settings = snapshot
            .settings
            .get(name)
            .ok_or_else(|| Error::new("settings_not_ready", "List Plugin Settings", "wait for the plugin to report its description"))?;
        let response = wire::SettingsListResponse {
            declarations: settings.declarations.clone(),
            value: Some(settings.value.clone()),
            revision: settings.revision.clone(),
            resolved_at: Some(snapshot.cut.clone()),
        };
        if response.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new("response_capacity", "List Plugin Settings", "configure capacity for the complete settings value"));
        }
        self.issued.insert(snapshot.cut.positions[0].sequence);
        Ok(response)
    }
    pub(super) async fn settings_set(&mut self, request: wire::SettingsSetRequest) -> Result<wire::SettingsSetResponse, Error> {
        if request.request_id.is_empty() {
            return Err(Error::new("identity_required", "Set Plugin Settings", "supply a nonempty request identity"));
        }
        let saved = self
            .registry
            .entries
            .get(&request.name)
            .ok_or_else(|| Error::new("entry_missing", "Set Plugin Settings", "select an existing plugin entry"))?;
        let encoded = normalized(&request);
        if let Some(original) = saved.settings_receipts.get(&request.request_id) {
            if original.request != encoded {
                return Err(Error::new(
                    "request_conflict",
                    "Set Plugin Settings",
                    "resend the original payload or choose a new request identity",
                ));
            }
            return wire::SettingsSetResponse::decode(original.response.as_slice())
                .map_err(|error| problem("Replay Plugin Settings", error));
        }
        let mut current = snapshots(&self.registry)?;
        let current = current
            .remove(&request.name)
            .ok_or_else(|| Error::new("settings_not_ready", "Set Plugin Settings", "wait for the plugin to report its description"))?;
        if current.revision != request.expected_revision {
            return Err(Error::new("revision_conflict", "Set Plugin Settings", "list settings again and retry at the returned revision"));
        }
        let value =
            request.value.as_ref().ok_or_else(|| Error::new("value_required", "Set Plugin Settings", "supply the whole declared value"))?;
        validate_values(&current.declarations, value)?;
        let retained = self.registry.creates.len()
            + self.registry.entries.values().map(|entry| entry.settings_receipts.len() + entry.replacements.len()).sum::<usize>();
        if retained >= self.settings.create_receipts.get() {
            return Err(Error::new("receipt_capacity", "Set Plugin Settings", "configure capacity for retained mutation receipts"));
        }
        let mut registry = self.registry.clone();
        let revision = registry.revision()?;
        let response = wire::SettingsSetResponse {
            value: Some(value.clone()),
            revision: revision.clone(),
            receipt: Some(self.receipt(request.request_id.clone())),
        };
        if response.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new(
                "response_capacity",
                "Set Plugin Settings",
                "configure capacity for the complete settings value and receipt",
            ));
        }
        let saved = registry.entries.get_mut(&request.name).expect("entry was selected");
        saved.settings = value.encode_to_vec();
        saved.settings_revision = revision;
        saved.settings_receipts.insert(
            request.request_id,
            SavedMutation {
                request: encoded,
                response: response.encode_to_vec(),
            },
        );
        let entry = PluginEntry::decode(saved.entry.as_slice()).map_err(|error| problem("Set Plugin Settings", error))?;
        self.commit(registry, changed(entry)).await?;
        Ok(response)
    }
}

/* === Public Functions === */

pub(crate) fn declarations_from(declarations: &Declarations) -> Result<Vec<SettingDeclaration>, Error> {
    let Some(section) = declarations.sections.iter().find(|section| section.r#type == "harmos.model.v1.SettingsDeclarations") else {
        return Ok(Vec::new());
    };
    let values = SettingsDeclarations::decode(section.payload.as_slice())
        .map_err(|error| problem("Read Plugin Setting Declarations", error))?
        .values;
    let mut names = BTreeSet::new();
    for declaration in &values {
        if declaration.name.is_empty() || !names.insert(&declaration.name) {
            return Err(Error::new("setting_declaration", "Declare Plugin Settings", "declare each nonempty name once"));
        }
        validate_value(
            declaration
                .default
                .as_ref()
                .ok_or_else(|| Error::new("setting_default", "Declare Plugin Settings", "supply a typed default"))?,
        )?;
    }
    Ok(values)
}

pub(super) fn materialize(registry: &mut Registry) -> Result<(), Error> {
    let names = registry.entries.keys().cloned().collect::<Vec<_>>();
    for name in names {
        let saved = registry.entries.get(&name).expect("listed entry exists");
        let entry = PluginEntry::decode(saved.entry.as_slice()).map_err(|error| problem("Read Plugin Settings", error))?;
        let Some(declarations) = entry.description.as_ref().and_then(|description| description.declarations.as_ref()) else {
            continue;
        };
        let declarations = declarations_from(declarations)?;
        let mut value = if saved.settings.is_empty() {
            SettingValues::default()
        } else {
            SettingValues::decode(saved.settings.as_slice()).map_err(|error| problem("Restore Plugin Settings", error))?
        };
        let original = value.clone();
        for declaration in &declarations {
            value.values.entry(declaration.name.clone()).or_insert_with(|| declaration.default.clone().expect("validated default"));
        }
        validate_values(&declarations, &value)?;
        let encoded = value.encode_to_vec();
        if value != original || saved.settings_revision.is_empty() {
            let revision = registry.revision()?;
            let saved = registry.entries.get_mut(&name).expect("listed entry exists");
            saved.settings = encoded;
            saved.settings_revision = revision;
        }
    }
    Ok(())
}

pub(crate) fn select(declarations: &Declarations, overrides: Option<SettingValues>) -> Result<SettingValues, Error> {
    let declarations = declarations_from(declarations)?;
    let mut value = overrides.unwrap_or_default();
    for declaration in &declarations {
        value.values.entry(declaration.name.clone()).or_insert_with(|| declaration.default.clone().expect("validated default"));
    }
    validate_values(&declarations, &value)?;
    Ok(value)
}

pub(super) fn snapshots(registry: &Registry) -> Result<BTreeMap<String, Snapshot>, Error> {
    let mut output = BTreeMap::new();
    for (name, saved) in &registry.entries {
        let entry = PluginEntry::decode(saved.entry.as_slice()).map_err(|error| problem("Read Plugin Settings", error))?;
        let Some(declarations) = entry.description.and_then(|description| description.declarations) else {
            continue;
        };
        let declarations = declarations_from(&declarations)?;
        let value = SettingValues::decode(saved.settings.as_slice()).map_err(|error| problem("Restore Plugin Settings", error))?;
        validate_values(&declarations, &value)?;
        output.insert(
            name.clone(),
            Snapshot {
                declarations,
                value,
                revision: saved.settings_revision.clone(),
            },
        );
    }
    Ok(output)
}

/* === Private Functions === */

fn normalized(request: &wire::SettingsSetRequest) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [request.name.as_bytes(), request.expected_revision.as_slice()] {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }
    bytes.push(u8::from(request.value.is_some()));
    if let Some(value) = &request.value {
        let ordered = value.values.iter().collect::<BTreeMap<_, _>>();
        for (name, value) in ordered {
            bytes.extend_from_slice(&(name.len() as u64).to_le_bytes());
            bytes.extend_from_slice(name.as_bytes());
            let value = value.encode_to_vec();
            bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&value);
        }
    }
    bytes
}

fn validate_values(declarations: &[SettingDeclaration], value: &SettingValues) -> Result<(), Error> {
    if declarations.len() != value.values.len() {
        return Err(Error::new("setting_names", "Set Plugin Settings", "supply exactly the declared setting names"));
    }
    for declaration in declarations {
        let value = value.values.get(&declaration.name).ok_or_else(|| {
            Error::new("setting_missing", format!("Set Plugin Setting {}", declaration.name), "supply the complete declared value")
        })?;
        validate_value(value)?;
        let expected = declaration.default.as_ref().and_then(|value| value.value.as_ref()).expect("validated default");
        if std::mem::discriminant(value.value.as_ref().expect("validated value")) != std::mem::discriminant(expected) {
            return Err(Error::new(
                "setting_type",
                format!("Set Plugin Setting {}", declaration.name),
                "use the declared default's value variant",
            ));
        }
    }
    Ok(())
}
fn validate_value(value: &SettingValue) -> Result<(), Error> {
    let valid = match &value.value {
        None => false,
        Some(Value::Number(value)) => value.is_finite(),
        Some(Value::Duration(value)) => {
            (-315_576_000_000..=315_576_000_000).contains(&value.seconds)
                && (-999_999_999..=999_999_999).contains(&value.nanos)
                && (value.seconds == 0 || value.nanos == 0 || value.seconds.signum() == i64::from(value.nanos).signum())
        }
        Some(_) => true,
    };
    if !valid {
        return Err(Error::new("setting_value", "Validate Plugin Setting", "select a finite, valid typed value"));
    }
    Ok(())
}

/* === Tests === */
