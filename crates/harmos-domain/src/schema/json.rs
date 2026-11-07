/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Schema-Driven JSON Values Use The Canonical Postcard Representation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde_json::{Map, Value};

// Local Imports
use crate::MessageError;
use crate::message::postcard::{Reader, Value as Postcard};
use crate::schema::Cardinality;
use crate::schema::graph::{Kind, Schema, View};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct JsonCodec {
    schema: Schema,
    root: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl JsonCodec {
    pub fn new(view: View<'_>, settings: &crate::config::SchemaSettings) -> Result<Self, MessageError> {
        view.schema().prepare(settings).map_err(invalid)?;
        Ok(Self {
            schema: view.schema().clone(),
            root: view.index(),
        })
    }

    pub fn encode(&self, value: &Value) -> Result<Vec<u8>, MessageError> {
        let mut bytes = Vec::new();
        let stored = !self.schema.representations.is_empty();
        let value = if stored { super::wire::encode(&self.schema, self.root, value)? } else { value.clone() };
        encode(&self.schema, self.root, &value, &mut bytes, 0)?;
        Ok(bytes)
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<Value, MessageError> {
        let mut reader = Reader::new(bytes);
        let value = decode(&self.schema, self.root, &mut reader)?;
        reader.finish()?;
        if self.schema.representations.is_empty() { Ok(value) } else { super::wire::decode(&self.schema, self.root, &value) }
    }

    pub fn memory(&self, bytes: &[u8]) -> Result<usize, MessageError> {
        let mut reader = Reader::new(bytes);
        let bytes = memory(&self.schema, self.root, &mut reader)?;
        reader.finish()?;
        Ok(bytes)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(error: impl std::fmt::Display) -> MessageError {
    MessageError::new(error.to_string())
}

fn encode(schema: &Schema, root: usize, value: &Value, bytes: &mut Vec<u8>, depth: usize) -> Result<(), MessageError> {
    if depth >= crate::schema::MAX_DEPTH {
        return Err(invalid("message nesting exceeds the structural limit"));
    }
    let fields = &schema.definitions[root];
    if fields.is_empty() {
        return if value.is_null() || value.as_object().is_some_and(Map::is_empty) {
            Ok(())
        } else {
            Err(invalid("unit value must be None"))
        };
    }
    let object = value.as_object().ok_or_else(|| invalid("message must be an object"))?;
    if object.keys().any(|name| !fields.iter().any(|field| field.name == name.as_str())) {
        return Err(invalid("message contains an undeclared field"));
    }
    if fields[0].cardinality == Cardinality::Variant {
        let (index, field) = fields
            .iter()
            .enumerate()
            .find(|(_, field)| object.contains_key(field.name.as_ref()))
            .ok_or_else(|| invalid("enum variant is absent"))?;
        if object.len() != 1 {
            return Err(invalid("enum requires exactly one variant"));
        }
        index.write(bytes);
        return encode_kind(schema, field.kind, &object[field.name.as_ref()], bytes, depth);
    }
    for field in fields {
        let value = object.get(field.name.as_ref()).unwrap_or(&Value::Null);
        match field.cardinality {
            Cardinality::Optional => {
                u8::from(!value.is_null()).write(bytes);
                if !value.is_null() {
                    encode_kind(schema, field.kind, value, bytes, depth)?;
                }
            }
            Cardinality::Repeated => {
                let values = value.as_array().ok_or_else(|| invalid(format!("{} must be a list", field.name)))?;
                values.len().write(bytes);
                for value in values {
                    encode_kind(schema, field.kind, value, bytes, depth)?;
                }
            }
            Cardinality::Required => encode_kind(schema, field.kind, value, bytes, depth)?,
            Cardinality::Variant => return Err(invalid("mixed enum and product fields")),
        }
    }
    Ok(())
}

fn encode_kind(schema: &Schema, kind: Kind, value: &Value, bytes: &mut Vec<u8>, depth: usize) -> Result<(), MessageError> {
    let mismatch = || invalid("value does not match its declared type");
    match kind {
        Kind::Bool => value.as_bool().ok_or_else(mismatch)?.write(bytes),
        Kind::U32 => u32::try_from(value.as_u64().ok_or_else(mismatch)?).map_err(invalid)?.write(bytes),
        Kind::U64 => value.as_u64().ok_or_else(mismatch)?.write(bytes),
        Kind::I32 => i32::try_from(value.as_i64().ok_or_else(mismatch)?).map_err(invalid)?.write(bytes),
        Kind::I64 => value.as_i64().ok_or_else(mismatch)?.write(bytes),
        Kind::F32 => {
            let number = value.as_f64().ok_or_else(mismatch)? as f32;
            if !number.is_finite() {
                return Err(mismatch());
            }
            number.write(bytes);
        }
        Kind::F64 => value.as_f64().filter(|value| value.is_finite()).ok_or_else(mismatch)?.write(bytes),
        Kind::String => value.as_str().ok_or_else(mismatch)?.to_owned().write(bytes),
        Kind::Bytes => serde_json::from_value::<Vec<u8>>(value.clone()).map_err(invalid)?.write(bytes),
        Kind::Message { definition } => encode(schema, definition, value, bytes, depth + 1)?,
    }
    Ok(())
}

fn decode(schema: &Schema, root: usize, reader: &mut Reader<'_>) -> Result<Value, MessageError> {
    reader.nested(|reader| {
        let fields = &schema.definitions[root];
        let mut object = Map::new();
        if fields.first().is_some_and(|field| field.cardinality == Cardinality::Variant) {
            let field = fields.get(usize::read(reader)?).ok_or_else(|| invalid("unknown enum variant"))?;
            object.insert(field.name.to_string(), decode_kind(schema, field.kind, reader)?);
            return Ok(Value::Object(object));
        }
        for field in fields {
            let value = match field.cardinality {
                Cardinality::Optional => match u8::read(reader)? {
                    0 => Value::Null,
                    1 => decode_kind(schema, field.kind, reader)?,
                    _ => return Err(invalid("invalid optional tag")),
                },
                Cardinality::Repeated => {
                    let count = usize::read(reader)?;
                    reader.claim(count)?;
                    let values = (0..count).map(|_| decode_kind(schema, field.kind, reader)).collect::<Result<Vec<_>, _>>()?;
                    Value::Array(values)
                }
                Cardinality::Required => decode_kind(schema, field.kind, reader)?,
                Cardinality::Variant => return Err(invalid("mixed enum and product fields")),
            };
            object.insert(field.name.to_string(), value);
        }
        Ok(Value::Object(object))
    })
}

fn decode_kind(schema: &Schema, kind: Kind, reader: &mut Reader<'_>) -> Result<Value, MessageError> {
    Ok(match kind {
        Kind::Bool => Value::from(bool::read(reader)?),
        Kind::U32 => Value::from(u32::read(reader)?),
        Kind::U64 => Value::from(u64::read(reader)?),
        Kind::I32 => Value::from(i32::read(reader)?),
        Kind::I64 => Value::from(i64::read(reader)?),
        Kind::F32 => Value::Number(serde_json::Number::from_f64(f32::read(reader)? as f64).ok_or_else(|| invalid("nonfinite float"))?),
        Kind::F64 => Value::Number(serde_json::Number::from_f64(f64::read(reader)?).ok_or_else(|| invalid("nonfinite float"))?),
        Kind::String => Value::from(String::read(reader)?),
        Kind::Bytes => serde_json::to_value(Vec::<u8>::read(reader)?).map_err(invalid)?,
        Kind::Message { definition } => decode(schema, definition, reader)?,
    })
}

fn memory(schema: &Schema, root: usize, reader: &mut Reader<'_>) -> Result<usize, MessageError> {
    reader.nested(|reader| {
        let fields = &schema.definitions[root];
        let mut bytes = std::mem::size_of::<Value>();
        if fields.first().is_some_and(|field| field.cardinality == Cardinality::Variant) {
            let field = fields.get(usize::read(reader)?).ok_or_else(|| invalid("unknown enum variant"))?;
            return Ok(bytes.saturating_add(memory_kind(schema, field.kind, reader)?).saturating_add(field.name.len()));
        }
        for field in fields {
            let count = match field.cardinality {
                Cardinality::Optional => match u8::read(reader)? {
                    0 => 0,
                    1 => 1,
                    _ => return Err(invalid("invalid optional tag")),
                },
                Cardinality::Repeated => usize::read(reader)?,
                Cardinality::Required => 1,
                Cardinality::Variant => return Err(invalid("mixed enum and product fields")),
            };
            reader.claim(count)?;
            bytes = bytes.saturating_add(field.name.len()).saturating_add(std::mem::size_of::<Value>());
            for _ in 0..count {
                bytes = bytes.saturating_add(memory_kind(schema, field.kind, reader)?);
            }
        }
        Ok(bytes.saturating_mul(2))
    })
}

fn memory_kind(schema: &Schema, kind: Kind, reader: &mut Reader<'_>) -> Result<usize, MessageError> {
    let bytes = match kind {
        Kind::Bool => bool::memory(reader)?,
        Kind::U32 => u32::memory(reader)?,
        Kind::U64 => u64::memory(reader)?,
        Kind::I32 => i32::memory(reader)?,
        Kind::I64 => i64::memory(reader)?,
        Kind::F32 => f32::memory(reader)?,
        Kind::F64 => f64::memory(reader)?,
        Kind::String => String::memory(reader)?,
        Kind::Bytes => Vec::<u8>::memory(reader)?,
        Kind::Message { definition } => return memory(schema, definition, reader),
    };
    Ok(bytes.saturating_mul(6).saturating_add(std::mem::size_of::<Value>()))
}

/* === Tests === */

#[cfg(all(test, feature = "derive"))]
mod tests {
    use super::*;
    use crate::schema::Described;
    use crate::{Message, message};
    use serde_json::json;

    #[message(harmos_path = crate)]
    struct Child {
        enabled: bool,
    }

    #[message(harmos_path = crate)]
    struct Row {
        counter: u64,
        voltage: Option<f64>,
        labels: Vec<String>,
        child: Child,
        data: Vec<u8>,
    }

    #[message(harmos_path = crate)]
    struct Unit {}

    #[message(harmos_path = crate)]
    enum Choice {
        Empty,
        Reading { value: i32 },
    }

    #[test]
    fn json_codec_matches_native_products_and_refuses_malformed_payloads() {
        let schema = Schema::from_declaration(Row::DEFINITION, &Default::default()).unwrap();
        let codec = JsonCodec::new(schema.root().unwrap(), &Default::default()).unwrap();
        let row = Row {
            counter: 17,
            voltage: Some(1.25),
            labels: vec!["a".into(), "b".into()],
            child: Child { enabled: true },
            data: vec![0, 128, 255],
        };
        let value = json!({"counter": 17, "voltage": 1.25, "labels": ["a", "b"], "child": {"enabled": true}, "data": [0, 128, 255]});
        let encoded = codec.encode(&value).unwrap();
        assert_eq!(encoded, row.encode());
        assert_eq!(codec.decode(&encoded).unwrap(), value);
        assert!(codec.memory(&encoded).unwrap() >= encoded.len());
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove("counter");
        assert!(codec.encode(&missing).is_err());
        let mut invalid = value.clone();
        invalid["counter"] = json!(-1);
        assert!(codec.encode(&invalid).is_err());
        invalid = value.clone();
        invalid["extra"] = json!(1);
        assert!(codec.encode(&invalid).is_err());
        assert!(codec.decode(&encoded[..encoded.len() - 1]).is_err());
        assert!(codec.memory(&encoded[..encoded.len() - 1]).is_err());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(codec.decode(&trailing).is_err());
        assert!(codec.memory(&trailing).is_err());
        let mut optional = value;
        optional["voltage"] = serde_json::Value::Null;
        let encoded = codec.encode(&optional).unwrap();
        assert_eq!(codec.decode(&encoded).unwrap(), optional);
    }

    #[test]
    fn json_codec_preserves_enum_selection_and_unit_values() {
        let schema = Schema::from_declaration(Choice::DEFINITION, &Default::default()).unwrap();
        let codec = JsonCodec::new(schema.root().unwrap(), &Default::default()).unwrap();
        for (native, value) in [(Choice::Empty, json!({"Empty": {}})), (Choice::Reading { value: -7 }, json!({"Reading": {"value": -7}}))] {
            let encoded = codec.encode(&value).unwrap();
            assert_eq!(encoded, native.encode());
            assert_eq!(codec.decode(&encoded).unwrap(), value);
        }
        assert!(codec.encode(&json!({"Empty": {}, "Reading": {"value": 7}})).is_err());
        assert!(codec.encode(&json!({"Unknown": {}})).is_err());
        assert!(codec.decode(&[127]).is_err());
        let schema = Schema::from_declaration(Unit::DEFINITION, &Default::default()).unwrap();
        let codec = JsonCodec::new(schema.root().unwrap(), &Default::default()).unwrap();
        assert_eq!(codec.encode(&json!({})).unwrap(), Unit {}.encode());
        assert_eq!(codec.decode(&[]).unwrap(), json!({}));
        assert!(codec.encode(&json!({"unexpected": 1})).is_err());
    }
}
