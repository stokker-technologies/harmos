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

// Standard Library Imports
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    mem,
};

// Third Party Imports
use prost::Message;

// Local Imports
use super::{
    Error,
    actor::{Actor, changed},
    persistence::{Registry, SavedMutation, problem},
};
use harmos_protocol::host::PluginEntry;
use harmos_protocol::host::plugins;
use harmos_protocol::model::Declarations;
use harmos_protocol::model::SettingDeclaration;
use harmos_protocol::model::SettingValue;
use harmos_protocol::model::SettingValues;
use harmos_protocol::model::SettingsDeclarations;
use harmos_protocol::model::setting_value::Value;

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
    pub(super) fn settings_list(&mut self, request: plugins::SettingsListRequest) -> Result<plugins::SettingsListResponse, Error> {
        let name = request
            .name
            .as_ref()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| Error::new("name_required", "List Plugin Settings", "select a plugin entry"))?;
        if !self.registry.entries.contains_key(name) {
            return Err(Error::new("entry_missing", "List Plugin Settings", "select an existing plugin entry"));
        }
        let snapshot = self.snapshot(request.at.as_ref())?.clone();
        let settings = snapshot.settings.get(name).ok_or_else(|| {
            Error::transient("settings_not_ready", "List Plugin Settings", "retry after the plugin reports its description")
        })?;
        let response = plugins::SettingsListResponse {
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
    pub(super) async fn settings_set(&mut self, request: plugins::SettingsSetRequest) -> Result<plugins::SettingsSetResponse, Error> {
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
            return plugins::SettingsSetResponse::decode(original.response.as_slice())
                .map_err(|error| problem("Replay Plugin Settings", error));
        }
        let mut current = snapshots(&self.registry)?;
        let current = current.remove(&request.name).ok_or_else(|| {
            Error::transient("settings_not_ready", "Set Plugin Settings", "retry after the plugin reports its description")
        })?;
        if current.revision != request.expected_revision {
            return Err(Error::new("revision_conflict", "Set Plugin Settings", "list settings again and retry at the returned revision"));
        }
        let mut value =
            request.value.clone().ok_or_else(|| Error::new("value_required", "Set Plugin Settings", "supply the whole declared value"))?;
        for declaration in &current.declarations {
            if !value.values.contains_key(&declaration.name) && !declaration.required {
                value.values.insert(declaration.name.clone(), declaration.default.clone().expect("validated default"));
            }
        }
        validate_values(&current.declarations, &value)?;
        let retained = self.registry.creates.len()
            + self.registry.entries.values().map(|entry| entry.settings_receipts.len() + entry.replacements.len()).sum::<usize>();
        if retained >= self.settings.create_receipts.get() {
            return Err(Error::new("receipt_capacity", "Set Plugin Settings", "configure capacity for retained mutation receipts"));
        }
        let mut registry = self.registry.clone();
        let revision = registry.revision()?;
        let response = plugins::SettingsSetResponse {
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
        let default = declaration
            .default
            .as_ref()
            .ok_or_else(|| Error::new("setting_default", "Declare Plugin Settings", "supply a typed default"))?;
        validate_setting(declaration, default)?;
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

fn normalized(request: &plugins::SettingsSetRequest) -> Vec<u8> {
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
    for declaration in declarations {
        let value = value.values.get(&declaration.name).ok_or_else(|| {
            Error::new(
                "setting_missing",
                format!("Set Plugin Setting {}", declaration.name),
                "include this required setting with a typed value",
            )
        })?;
        validate_setting(declaration, value)?;
    }
    for name in value.values.keys() {
        if !declarations.iter().any(|declaration| declaration.name == *name) {
            return Err(Error::new("setting_names", format!("Set Plugin Setting {name}"), "remove this undeclared setting"));
        }
    }
    Ok(())
}

fn validate_setting(declaration: &SettingDeclaration, value: &SettingValue) -> Result<(), Error> {
    let action = format!("Validate Plugin Setting {}", declaration.name);
    validate_value(value, &action)?;
    let expected = declaration
        .default
        .as_ref()
        .and_then(|value| value.value.as_ref())
        .ok_or_else(|| Error::new("setting_default", &action, "supply a typed default"))?;
    if mem::discriminant(value.value.as_ref().expect("validated value")) != mem::discriminant(expected) {
        return Err(Error::new("setting_type", &action, "use the declared default's value variant"));
    }
    for choice in &declaration.choices {
        validate_value(choice, &action)?;
        if mem::discriminant(choice.value.as_ref().expect("validated choice")) != mem::discriminant(expected) {
            return Err(Error::new("setting_choices", &action, "declare choices with the default's value variant"));
        }
    }
    if !declaration.choices.is_empty() && !declaration.choices.contains(value) {
        return Err(Error::new("setting_choice", &action, &format!("choose one of {:?}", declaration.choices)));
    }
    for bound in [&declaration.minimum, &declaration.maximum].into_iter().flatten() {
        validate_value(bound, &action)?;
        if compare(expected, bound.value.as_ref().expect("validated bound")).is_none() {
            return Err(Error::new("setting_bounds", &action, "use the default's integer, number or duration variant for bounds"));
        }
    }
    if let (Some(minimum), Some(maximum)) = (&declaration.minimum, &declaration.maximum)
        && compare(minimum.value.as_ref().unwrap(), maximum.value.as_ref().unwrap()) == Some(Ordering::Greater)
    {
        return Err(Error::new("setting_bounds", &action, "set minimum less than or equal to maximum"));
    }
    let value = value.value.as_ref().expect("validated value");
    if let Some(minimum) = &declaration.minimum
        && compare(value, minimum.value.as_ref().unwrap()) == Some(Ordering::Less)
    {
        return Err(Error::new(
            "setting_minimum",
            &action,
            &format!("supply a value at or above minimum {:?}", minimum.value.as_ref().unwrap()),
        ));
    }
    if let Some(maximum) = &declaration.maximum
        && compare(value, maximum.value.as_ref().unwrap()) == Some(Ordering::Greater)
    {
        return Err(Error::new(
            "setting_maximum",
            &action,
            &format!("supply a value at or below maximum {:?}", maximum.value.as_ref().unwrap()),
        ));
    }
    Ok(())
}

fn compare(value: &Value, bound: &Value) -> Option<Ordering> {
    match (value, bound) {
        (Value::Integer(value), Value::Integer(bound)) => Some(value.cmp(bound)),
        (Value::Number(value), Value::Number(bound)) => value.partial_cmp(bound),
        (Value::Duration(value), Value::Duration(bound)) => Some((value.seconds, value.nanos).cmp(&(bound.seconds, bound.nanos))),
        _ => None,
    }
}
fn validate_value(value: &SettingValue, action: &str) -> Result<(), Error> {
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
        return Err(Error::new("setting_value", action, "select a finite, valid typed value"));
    }
    Ok(())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use harmos_protocol::model::Section;

    #[test]
    fn choices_and_missing_required_values_name_the_setting_and_fix() {
        let mut declaration = declaration(Value::Integer(7), Value::Integer(0), Value::Integer(10));
        declaration.required = true;
        declaration.choices = [7, 9].map(|value| SettingValue { value: Some(Value::Integer(value)) }).into();
        validate_setting(&declaration, &SettingValue { value: Some(Value::Integer(9)) }).unwrap();
        let error = validate_setting(&declaration, &SettingValue { value: Some(Value::Integer(8)) }).unwrap_err();
        assert_eq!(error.0.code, "setting_choice");
        assert!(error.0.message.contains("limit"));
        assert!(error.0.action.contains("choose one of"));
        assert!(error.0.action.contains('7') && error.0.action.contains('9'));
        let error = validate_values(&[declaration], &SettingValues::default()).unwrap_err();
        assert_eq!(error.0.code, "setting_missing");
        assert!(error.0.message.contains("limit"));
        assert!(error.0.action.contains("include this required setting"));
    }

    fn declaration(default: Value, minimum: Value, maximum: Value) -> SettingDeclaration {
        SettingDeclaration {
            name: "limit".into(),
            default: Some(SettingValue { value: Some(default) }),
            minimum: Some(SettingValue { value: Some(minimum) }),
            maximum: Some(SettingValue { value: Some(maximum) }),
            ..Default::default()
        }
    }

    #[test]
    fn bounds_are_typed_ordered_and_inclusive() {
        for (default, minimum, maximum) in [
            (Value::Integer(7), Value::Integer(0), Value::Integer(10)),
            (Value::Number(0.5), Value::Number(-1.0), Value::Number(1.0)),
            (
                Value::Duration(prost_types::Duration { seconds: 0, nanos: 0 }),
                Value::Duration(prost_types::Duration { seconds: -1, nanos: -1 }),
                Value::Duration(prost_types::Duration { seconds: 0, nanos: 1 }),
            ),
        ] {
            let declaration = declaration(default, minimum, maximum);
            validate_setting(&declaration, declaration.default.as_ref().unwrap()).unwrap();
            validate_setting(&declaration, declaration.minimum.as_ref().unwrap()).unwrap();
            validate_setting(&declaration, declaration.maximum.as_ref().unwrap()).unwrap();
            let mut reversed = declaration.clone();
            mem::swap(&mut reversed.minimum, &mut reversed.maximum);
            assert!(validate_setting(&reversed, reversed.default.as_ref().unwrap()).is_err());
        }
    }

    #[test]
    fn invalid_defaults_values_and_bounds_are_refused() {
        let valid = declaration(Value::Integer(7), Value::Integer(0), Value::Integer(10));
        for value in [Value::Integer(-1), Value::Integer(11), Value::Number(7.0)] {
            assert!(validate_setting(&valid, &SettingValue { value: Some(value) }).is_err());
        }
        for invalid in [
            declaration(Value::Integer(-1), Value::Integer(0), Value::Integer(10)),
            declaration(Value::Integer(7), Value::Number(0.0), Value::Integer(10)),
            declaration(Value::Number(0.0), Value::Number(f64::NAN), Value::Number(10.0)),
            declaration(Value::Flag(false), Value::Flag(false), Value::Flag(true)),
            declaration(Value::Text("a".into()), Value::Text("a".into()), Value::Text("z".into())),
        ] {
            let declarations = Declarations {
                sections: vec![Section {
                    r#type: "harmos.model.v1.SettingsDeclarations".into(),
                    payload: SettingsDeclarations { values: vec![invalid] }.encode_to_vec(),
                }],
                ..Default::default()
            };
            assert!(select(&declarations, None).is_err());
        }
    }
}
