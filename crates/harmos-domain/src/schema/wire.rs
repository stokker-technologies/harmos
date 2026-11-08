/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serde Wire Representations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;

// Third Party Imports
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

// Local Imports
use super::Cardinality;
use super::graph::{Field, Kind, Schema};
use crate::MessageError;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Representation {
    pub form: Form,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub omissions: BTreeMap<String, Omission>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maps: Vec<String>,
}

pub(crate) struct Seed<'a> {
    pub remaining: &'a mut usize,
}

struct Encoded(String);

/* === Enums === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Form {
    #[default]
    Object,
    Unit,
    Value,
    Tuple,
    External,
    Internal,
    Adjacent,
    Untagged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Omission {
    None,
    Empty,
}

/* === Traits === */

/* === Implementations === */

impl<'de> Deserialize<'de> for Encoded {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Input;
        impl Visitor<'_> for Input {
            type Value = Encoded;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("bounded Serde representation metadata")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Encoded, E> {
                if value.len() > crate::FRAGMENT_LIMIT {
                    return Err(E::custom("Serde metadata exceeds its bound"));
                }
                Ok(Encoded(value.to_owned()))
            }
        }
        deserializer.deserialize_str(Input)
    }
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = BTreeMap<usize, Representation>;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Seed<'_> {
    type Value = BTreeMap<usize, Representation>;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("bounded Serde wire representations")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
        let mut output = BTreeMap::new();
        while let Some((index, Encoded(encoded))) = input.next_entry::<usize, Encoded>()? {
            let cost = encoded
                .len()
                .checked_mul(8)
                .and_then(|cost| cost.checked_add(4 * std::mem::size_of::<Representation>()))
                .ok_or_else(|| A::Error::custom("Serde metadata budget"))?;
            *self.remaining = self.remaining.checked_sub(cost).ok_or_else(|| A::Error::custom("Serde metadata budget"))?;
            let representation = serde_json::from_str(&encoded).map_err(A::Error::custom)?;
            if output.insert(index, representation).is_some() {
                return Err(A::Error::custom("duplicate Serde representation"));
            }
        }
        Ok(output)
    }
}

impl Representation {
    pub(crate) fn is_value(&self, fields: &[Field]) -> bool {
        self.form == Form::Value
            && self.tag.is_none()
            && self.content.is_none()
            && self.omissions.is_empty()
            && self.maps.is_empty()
            && fields.len() == 1
            && fields[0].name == "value"
            && fields[0].tag == 1
    }

    pub fn parse(encoded: &str) -> Result<Option<Self>, super::graph::Error> {
        if encoded.is_empty() {
            return Ok(None);
        }
        if encoded.len() > crate::FRAGMENT_LIMIT {
            return Err(super::graph::Error::Budget);
        }
        serde_json::from_str(encoded).map(Some).map_err(|_| super::graph::Error::Field {
            field: "serde".into(),
            reason: "invalid Serde wire representation",
        })
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        let names = self.omissions.keys().map(String::capacity).sum::<usize>();
        let maps = self.maps.iter().map(String::capacity).sum::<usize>();
        self.tag
            .as_ref()
            .map_or(0, String::capacity)
            .saturating_add(self.content.as_ref().map_or(0, String::capacity))
            .saturating_add(names)
            .saturating_add(maps)
            .saturating_add(self.maps.capacity().saturating_mul(std::mem::size_of::<String>()))
            .saturating_add(self.omissions.len().saturating_mul(4 * std::mem::size_of::<(String, Omission)>()))
            .saturating_add(4 * std::mem::size_of::<(usize, Self)>())
    }
}

/* === Public Functions === */

pub const fn tag_available(fields: &[Field<super::declaration::Definition>], own_tag: Option<&str>, tag: &str) -> bool {
    const fn same(left: &str, right: &str) -> bool {
        if left.len() != right.len() {
            return false;
        }
        let mut index = 0;
        while index < left.len() {
            if left.as_bytes()[index] != right.as_bytes()[index] {
                return false;
            }
            index += 1;
        }
        true
    }
    if let Some(own_tag) = own_tag
        && same(own_tag, tag)
    {
        return false;
    }
    let mut index = 0;
    while index < fields.len() {
        if let std::borrow::Cow::Borrowed(name) = &fields[index].name
            && same(name, tag)
        {
            return false;
        }
        index += 1;
    }
    true
}

pub fn serialize<S: serde::Serializer>(representations: &BTreeMap<usize, Representation>, serializer: S) -> Result<S::Ok, S::Error> {
    let mut output = serializer.serialize_map(Some(representations.len()))?;
    for (index, representation) in representations {
        let encoded = serde_json::to_string(representation).map_err(serde::ser::Error::custom)?;
        output.serialize_entry(index, &encoded)?;
    }
    output.end()
}

pub(crate) fn check(representation: &Representation, fields: &[Field]) -> Result<(), super::graph::Error> {
    let invalid = || super::graph::Error::Field {
        field: "serde".into(),
        reason: "wire representation conflicts with fields",
    };
    let variants = fields.iter().all(|field| field.cardinality == Cardinality::Variant);
    match representation.form {
        Form::Unit if !fields.is_empty() => return Err(invalid()),
        Form::Value if fields.len() != 1 => return Err(invalid()),
        Form::External | Form::Untagged | Form::Internal | Form::Adjacent if !variants => return Err(invalid()),
        _ => {}
    }
    let tagged = matches!(representation.form, Form::Internal | Form::Adjacent);
    if tagged != representation.tag.is_some() || (representation.form == Form::Adjacent) != representation.content.is_some() {
        return Err(invalid());
    }
    if representation.tag.as_ref().is_some_and(|tag| tag.len() > super::MAX_NAME_BYTES)
        || representation.content.as_ref().is_some_and(|content| content.len() > super::MAX_NAME_BYTES)
        || representation.tag.is_some() && representation.tag == representation.content
    {
        return Err(invalid());
    }
    for (name, omission) in &representation.omissions {
        let field = fields.iter().find(|field| field.name == name.as_str()).ok_or_else(invalid)?;
        let valid = match omission {
            Omission::None => field.cardinality == Cardinality::Optional,
            Omission::Empty => field.cardinality == Cardinality::Repeated || field.kind == Kind::Bytes,
        };
        if !valid || representation.form != Form::Object {
            return Err(invalid());
        }
    }
    for name in &representation.maps {
        let field = fields.iter().find(|field| field.name == name.as_str()).ok_or_else(invalid)?;
        if field.cardinality != Cardinality::Repeated || !matches!(field.kind, Kind::Message { .. }) {
            return Err(invalid());
        }
    }
    Ok(())
}

// Translate The Stored Serde Form To The Schema's Positional Value Tree.
pub(crate) fn encode(schema: &Schema, root: usize, value: &Value) -> Result<Value, MessageError> {
    transform(schema, root, value, true, 0)
}

pub(crate) fn decode(schema: &Schema, root: usize, value: &Value) -> Result<Value, MessageError> {
    transform(schema, root, value, false, 0)
}

/* === Private Functions === */

fn invalid() -> MessageError {
    MessageError::borrowed("value does not match its Serde wire schema")
}

fn transform(schema: &Schema, root: usize, value: &Value, encoding: bool, depth: usize) -> Result<Value, MessageError> {
    if depth >= super::MAX_DEPTH {
        return Err(invalid());
    }
    let fields = &schema.definitions[root];
    let Some(representation) = schema.representations.get(&root) else {
        return object(schema, fields, &Representation::default(), value, encoding, depth);
    };
    match representation.form {
        Form::Unit => {
            if encoding && !value.is_null() {
                return Err(invalid());
            }
            Ok(if encoding { Value::Object(Map::new()) } else { Value::Null })
        }
        Form::Value => {
            let field = fields.first().ok_or_else(invalid)?;
            if encoding {
                let inner = member(schema, field, representation, value, encoding, depth)?;
                Ok(Value::Object(Map::from_iter([(field.name.to_string(), inner)])))
            } else {
                let value = value.get(field.name.as_ref()).ok_or_else(invalid)?;
                member(schema, field, representation, value, encoding, depth)
            }
        }
        Form::Tuple => {
            if encoding {
                let values = value.as_array().filter(|values| values.len() == fields.len()).ok_or_else(invalid)?;
                let object = fields
                    .iter()
                    .zip(values)
                    .map(|(field, value)| {
                        member(schema, field, representation, value, encoding, depth).map(|value| (field.name.to_string(), value))
                    })
                    .collect::<Result<Map<_, _>, _>>()?;
                Ok(Value::Object(object))
            } else {
                let values = fields
                    .iter()
                    .map(|field| {
                        member(schema, field, representation, value.get(field.name.as_ref()).ok_or_else(invalid)?, encoding, depth)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Array(values))
            }
        }
        Form::Object => object(schema, fields, representation, value, encoding, depth),
        _ => enumeration(schema, fields, representation, value, encoding, depth),
    }
}

fn object(
    schema: &Schema, fields: &[Field], representation: &Representation, value: &Value, encoding: bool, depth: usize,
) -> Result<Value, MessageError> {
    let input = value.as_object().ok_or_else(invalid)?;
    if input.keys().any(|name| !fields.iter().any(|field| field.name == name.as_str())) {
        return Err(invalid());
    }
    let mut output = Map::new();
    for field in fields {
        let omission = representation.omissions.get(field.name.as_ref());
        let fallback = match omission {
            Some(Omission::None) => Value::Null,
            Some(Omission::Empty) => Value::Array(Vec::new()),
            None => Value::Null,
        };
        let value = input.get(field.name.as_ref());
        if encoding && value.is_none() && omission.is_none() {
            return Err(invalid());
        }
        let value = value.unwrap_or(&fallback);
        let omitted = match omission {
            Some(Omission::None) => value.is_null(),
            Some(Omission::Empty) => value.as_array().is_some_and(Vec::is_empty),
            None => false,
        };
        if encoding && input.contains_key(field.name.as_ref()) && omitted && omission == Some(&Omission::Empty) {
            return Err(invalid());
        }
        if !encoding && omitted {
            continue;
        }
        let mapped = if encoding && omission == Some(&Omission::None) && input.contains_key(field.name.as_ref()) {
            kind(schema, field.kind, value, true, depth)?
        } else {
            member(schema, field, representation, value, encoding, depth)?
        };
        output.insert(field.name.to_string(), mapped);
    }
    Ok(Value::Object(output))
}

fn member(
    schema: &Schema, field: &Field, representation: &Representation, value: &Value, encoding: bool, depth: usize,
) -> Result<Value, MessageError> {
    if representation.maps.iter().any(|name| name == field.name.as_ref()) {
        map(schema, field, value, encoding, depth)
    } else {
        field_value(schema, field, value, encoding, depth)
    }
}

fn field_value(schema: &Schema, field: &Field, value: &Value, encoding: bool, depth: usize) -> Result<Value, MessageError> {
    if field.cardinality == Cardinality::Optional && value.is_null() {
        return Ok(Value::Null);
    }
    if field.cardinality == Cardinality::Repeated {
        let values = value.as_array().ok_or_else(invalid)?;
        return values
            .iter()
            .map(|value| kind(schema, field.kind, value, encoding, depth))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    kind(schema, field.kind, value, encoding, depth)
}

fn kind(schema: &Schema, kind: Kind, value: &Value, encoding: bool, depth: usize) -> Result<Value, MessageError> {
    if let Kind::Message { definition } = kind {
        return transform(schema, definition, value, encoding, depth + 1);
    }
    let valid = match kind {
        Kind::Bool => value.is_boolean(),
        Kind::U32 => value.as_u64().is_some_and(|value| u32::try_from(value).is_ok()),
        Kind::U64 => value.as_u64().is_some(),
        Kind::I32 => value.as_i64().is_some_and(|value| i32::try_from(value).is_ok()),
        Kind::I64 => value.as_i64().is_some(),
        Kind::F32 | Kind::F64 => value.is_number(),
        Kind::String => value.is_string(),
        Kind::Bytes => value.as_array().is_some_and(|values| values.iter().all(|value| value.as_u64().is_some_and(|value| value <= 255))),
        Kind::Message { .. } => unreachable!(),
    };
    if valid { Ok(value.clone()) } else { Err(invalid()) }
}

fn map(schema: &Schema, field: &Field, value: &Value, encoding: bool, depth: usize) -> Result<Value, MessageError> {
    let Kind::Message { definition } = field.kind else {
        return Err(invalid());
    };
    let fields = &schema.definitions[definition];
    if fields.len() != 2 {
        return Err(invalid());
    }
    if encoding {
        let input = value.as_object().ok_or_else(invalid)?;
        let mut output = Vec::new();
        for (key, value) in input {
            let key = match fields[0].kind {
                Kind::String => Value::String(key.clone()),
                _ => serde_json::from_str(key).map_err(|_| invalid())?,
            };
            let value = field_value(schema, &fields[1], value, true, depth + 1)?;
            output.push(Value::Object(Map::from_iter([("key".into(), key), ("value".into(), value)])));
        }
        Ok(Value::Array(output))
    } else {
        let input = value.as_array().ok_or_else(invalid)?;
        let mut output = Map::new();
        for entry in input {
            let key = &entry["key"];
            let key = key.as_str().map(str::to_owned).unwrap_or_else(|| key.to_string());
            let value = field_value(schema, &fields[1], &entry["value"], false, depth + 1)?;
            output.insert(key, value);
        }
        Ok(Value::Object(output))
    }
}

fn enumeration(
    schema: &Schema, fields: &[Field], representation: &Representation, value: &Value, encoding: bool, depth: usize,
) -> Result<Value, MessageError> {
    if encoding && representation.form == Form::Untagged {
        for field in fields {
            if let Ok(payload) = kind(schema, field.kind, value, true, depth) {
                return Ok(Value::Object(Map::from_iter([(field.name.to_string(), payload)])));
            }
        }
        return Err(invalid());
    }
    let tag = representation.tag.as_deref().unwrap_or("");
    let content = representation.content.as_deref().unwrap_or("");
    let name = if !encoding {
        value.as_object().filter(|object| object.len() == 1).and_then(|object| object.keys().next()).map(String::as_str)
    } else if representation.form == Form::External {
        value
            .as_str()
            .or_else(|| value.as_object().filter(|object| object.len() == 1).and_then(|object| object.keys().next()).map(String::as_str))
    } else {
        value.get(tag).and_then(Value::as_str)
    }
    .ok_or_else(invalid)?;
    let field = fields.iter().find(|field| field.name == name).ok_or_else(invalid)?;
    let Kind::Message { definition } = field.kind else {
        return Err(invalid());
    };
    let unit = schema.representations.get(&definition).is_some_and(|representation| representation.form == Form::Unit);
    if encoding {
        let payload = match representation.form {
            Form::External if unit && value.is_string() => Value::Null,
            Form::External if !unit && value.is_object() => value[name].clone(),
            Form::Internal => {
                let mut payload = value.as_object().ok_or_else(invalid)?.clone();
                payload.remove(tag);
                if unit {
                    if !payload.is_empty() {
                        return Err(invalid());
                    }
                    Value::Null
                } else {
                    Value::Object(payload)
                }
            }
            Form::Adjacent => {
                let object = value.as_object().ok_or_else(invalid)?;
                if object.keys().any(|key| key != tag && key != content) {
                    return Err(invalid());
                }
                if unit {
                    if object.contains_key(content) {
                        return Err(invalid());
                    }
                    Value::Null
                } else {
                    object.get(content).ok_or_else(invalid)?.clone()
                }
            }
            _ => return Err(invalid()),
        };
        let payload = kind(schema, field.kind, &payload, true, depth)?;
        return Ok(Value::Object(Map::from_iter([(name.to_owned(), payload)])));
    }
    let payload = kind(schema, field.kind, &value[name], false, depth)?;
    match representation.form {
        Form::External if unit => Ok(Value::String(name.to_owned())),
        Form::External => Ok(Value::Object(Map::from_iter([(name.to_owned(), payload)]))),
        Form::Untagged => Ok(payload),
        Form::Internal => {
            let mut object = if unit { Map::new() } else { payload.as_object().ok_or_else(invalid)?.clone() };
            if object.insert(tag.to_owned(), Value::String(name.to_owned())).is_some() {
                return Err(invalid());
            }
            Ok(Value::Object(object))
        }
        Form::Adjacent => {
            let mut object = Map::from_iter([(tag.to_owned(), Value::String(name.to_owned()))]);
            if !unit {
                object.insert(content.to_owned(), payload);
            }
            Ok(Value::Object(object))
        }
        _ => Err(invalid()),
    }
}

/* === Tests === */
