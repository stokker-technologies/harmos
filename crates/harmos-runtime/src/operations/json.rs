/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host-Owned Protobuf JSON Invocation Conversion

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use prost::Message;
use prost_reflect::{Cardinality, DynamicMessage, Kind, MessageDescriptor, ReflectMessage, Value};

// Local Imports
use crate::{Error, Result};
use harmos_operations::Failure;
use harmos_protocol::model::{self, Classification};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn decode(operation: &str, descriptor: MessageDescriptor, json: &str) -> Result<Vec<u8>> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    let message = DynamicMessage::deserialize(descriptor.clone(), &mut deserializer).map_err(|error| {
        let reason = serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|value| diagnose(&descriptor, &value, ""))
            .unwrap_or_else(|| format!("Request Must Match {}: {error}", descriptor.full_name()));
        invalid(operation, reason)
    })?;
    deserializer.end().map_err(|error| invalid(operation, format!("Request Must Contain One JSON Value: {error}")))?;
    required(operation, &message, "")?;
    Ok(message.encode_to_vec())
}

pub(super) fn encode(operation: &str, descriptor: MessageDescriptor, payload: &[u8]) -> Result<Vec<u8>> {
    let message = DynamicMessage::decode(descriptor, payload)
        .map_err(|error| invalid(operation, format!("Reply Must Match Its Served Contract: {error}")))?;
    serde_json::to_vec(&message).map_err(|error| invalid(operation, format!("Reply Must Encode as Protobuf JSON: {error}")))
}

pub(super) fn unserved(operation: &str) -> Error {
    failure(operation, Classification::Refused, "No Plugin Serves This Operation; Install or Enable the Plugin That Provides It".into())
}

/* === Private Functions === */

fn invalid(operation: &str, reason: String) -> Error {
    failure(operation, Classification::Invalid, reason)
}

fn failure(operation: &str, classification: Classification, reason: String) -> Error {
    let wire = model::Failure {
        classification: classification as i32,
        message: format!("Call {operation}: {reason}"),
        operation: operation.into(),
        code: "operation_payload".into(),
        ..Default::default()
    };
    Error::OperationFailed {
        operation: operation.into(),
        source: Arc::new(Failure::from(wire)),
    }
}

// Run Only After Standard Mapping Rejects the Request; Reflection Remains the Parser.
fn diagnose(descriptor: &MessageDescriptor, value: &serde_json::Value, prefix: &str) -> Option<String> {
    let object = value.as_object()?;
    if descriptor.full_name().starts_with("google.protobuf.") && descriptor.full_name() != "google.protobuf.Empty" {
        return None;
    }
    for (name, value) in object {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}.{name}") };
        let Some(field) = descriptor.get_field_by_name(name).or_else(|| descriptor.get_field_by_json_name(name)) else {
            let fields = descriptor.fields().map(|field| field.name().to_owned()).collect::<Vec<_>>().join(", ");
            let fields = if fields.is_empty() { "No Fields" } else { &fields };
            return Some(format!("Field \"{path}\" Is Not in Its Request; Expected {fields}"));
        };
        let probe = serde_json::json!({name: value});
        if DynamicMessage::deserialize(descriptor.clone(), &probe).is_ok() {
            continue;
        }
        if let Kind::Message(nested) = field.kind() {
            if field.is_map() {
                if let Kind::Message(mapped) = nested.get_field(2).expect("Map Value Field").kind()
                    && let Some(values) = value.as_object()
                {
                    for (key, value) in values {
                        if let Some(reason) = diagnose(&mapped, value, &format!("{path}[{key}]")) {
                            return Some(reason);
                        }
                    }
                }
            } else if field.is_list() {
                if let Some(values) = value.as_array() {
                    for (index, value) in values.iter().enumerate() {
                        if let Some(reason) = diagnose(&nested, value, &format!("{path}[{index}]")) {
                            return Some(reason);
                        }
                    }
                }
            } else {
                if let Some(reason) = diagnose(&nested, value, &path) {
                    return Some(reason);
                }
            }
        }
        let kind = match field.kind() {
            Kind::Message(message) => message.full_name().to_owned(),
            Kind::Enum(enumeration) => format!("Enum {}", enumeration.full_name()),
            kind => {
                let mut name = format!("{kind:?}");
                name.get_mut(..1).expect("Protobuf Scalar Kind").make_ascii_uppercase();
                name
            }
        };
        let expected = if field.is_map() {
            format!("Object ({kind})")
        } else if field.is_list() {
            format!("Array of {kind}")
        } else {
            kind
        };
        return Some(format!("Field \"{path}\" Must Be {expected}"));
    }
    None
}

fn required(operation: &str, message: &DynamicMessage, prefix: &str) -> Result<()> {
    for field in message.descriptor().fields() {
        let path = if prefix.is_empty() { field.name().to_owned() } else { format!("{prefix}.{}", field.name()) };
        if field.cardinality() == Cardinality::Required && !message.has_field(&field) {
            return Err(invalid(operation, format!("Field \"{path}\" Is Required")));
        }
        if !message.has_field(&field) {
            continue;
        }
        match message.get_field(&field).as_ref() {
            Value::Message(nested) => required(operation, nested, &path)?,
            Value::List(values) => {
                for (index, value) in values.iter().enumerate() {
                    if let Value::Message(nested) = value {
                        required(operation, nested, &format!("{path}[{index}]"))?;
                    }
                }
            }
            Value::Map(values) => {
                for (key, value) in values {
                    if let Value::Message(nested) = value {
                        required(operation, nested, &format!("{path}[{key:?}]"))?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost_reflect::DescriptorPool;
    use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, MessageOptions};
    use serde_json::json;

    fn descriptor() -> MessageDescriptor {
        let fields = [("radio", 9, 1), ("sample_count", 5, 2)]
            .into_iter()
            .enumerate()
            .map(|(index, (name, kind, label))| FieldDescriptorProto {
                name: Some(name.into()),
                number: Some(index as i32 + 1),
                label: Some(label),
                r#type: Some(kind),
                ..Default::default()
            })
            .collect();
        let types = FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("request.proto".into()),
                package: Some("fixture".into()),
                syntax: Some("proto2".into()),
                message_type: vec![DescriptorProto {
                    name: Some("Request".into()),
                    field: fields,
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        DescriptorPool::from_file_descriptor_set(types).unwrap().get_message_by_name("fixture.Request").unwrap()
    }

    fn assert_failure(error: Error, classification: Classification, text: &str) {
        let Error::OperationFailed { source, .. } = error else { panic!("Expected an Operation Failure") };
        let failure = source.downcast_ref::<Failure>().unwrap();
        let wire = model::Failure::from(failure);
        assert_eq!(wire.classification, classification as i32);
        assert_eq!(wire.message, text);
    }

    #[test]
    fn json_round_trip_uses_standard_field_mapping_and_presence() {
        for text in [r#"{"radio":"wifi","sample_count":0}"#, r#"{"radio":"wifi","sampleCount":0}"#] {
            let payload = decode("fixture.call", descriptor(), text).unwrap();
            let reply = encode("fixture.call", descriptor(), &payload).unwrap();
            assert_eq!(serde_json::from_slice::<serde_json::Value>(&reply).unwrap(), json!({"radio":"wifi", "sampleCount":0}));
        }
    }

    #[test]
    fn json_failures_name_the_operation_field_and_required_action() {
        for (text, reason) in [
            (r#"{"extra":true,"sampleCount":1}"#, "Field \"extra\" Is Not in Its Request; Expected radio, sample_count"),
            (r#"{"radio":false,"sampleCount":1}"#, "Field \"radio\" Must Be String"),
            (r#"{"radio":"wifi"}"#, "Field \"sample_count\" Is Required"),
            (r#"{"sampleCount":null}"#, "Field \"sample_count\" Is Required"),
        ] {
            let error = decode("fixture.call", descriptor(), text).unwrap_err();
            assert_failure(error, Classification::Invalid, &format!("Call fixture.call: {reason}"));
        }
        assert_failure(
            unserved("fixture.absent"),
            Classification::Refused,
            "Call fixture.absent: No Plugin Serves This Operation; Install or Enable the Plugin That Provides It",
        );
    }

    #[test]
    fn json_rejects_trailing_values_and_preserves_well_known_types() {
        assert!(decode("fixture.call", descriptor(), r#"{"sampleCount":1} {}"#).is_err());
        let duration = model::SettingValue::default().descriptor().parent_pool().get_message_by_name("google.protobuf.Duration").unwrap();
        let payload = decode("fixture.duration", duration.clone(), r#""1.500s""#).unwrap();
        let reply = encode("fixture.duration", duration, &payload).unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&reply).unwrap(), json!("1.500s"));
    }

    #[test]
    fn json_unknown_fields_identify_map_values_and_empty_requests() {
        let mut file = descriptor().parent_file().file_descriptor_proto().clone();
        let request = &mut file.message_type[0];
        request.nested_type.push(DescriptorProto {
            name: Some("RadiosEntry".into()),
            options: Some(MessageOptions {
                map_entry: Some(true),
                ..Default::default()
            }),
            field: vec![
                FieldDescriptorProto {
                    name: Some("key".into()),
                    number: Some(1),
                    label: Some(1),
                    r#type: Some(9),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("value".into()),
                    number: Some(2),
                    label: Some(1),
                    r#type: Some(11),
                    type_name: Some(".fixture.Request".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        request.field.push(FieldDescriptorProto {
            name: Some("radios".into()),
            number: Some(3),
            label: Some(3),
            r#type: Some(11),
            type_name: Some(".fixture.Request.RadiosEntry".into()),
            ..Default::default()
        });
        let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet { file: vec![file] }).unwrap();
        let error = decode(
            "fixture.call",
            pool.get_message_by_name("fixture.Request").unwrap(),
            r#"{"sampleCount":1,"radios":{"home":{"sampleCount":2,"extra":true}}}"#,
        )
        .unwrap_err();
        assert_failure(
            error,
            Classification::Invalid,
            "Call fixture.call: Field \"radios[home].extra\" Is Not in Its Request; Expected radio, sample_count, radios",
        );

        let types = FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("google/protobuf/empty.proto".into()),
                package: Some("google.protobuf".into()),
                syntax: Some("proto3".into()),
                message_type: vec![DescriptorProto {
                    name: Some("Empty".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        let pool = DescriptorPool::from_file_descriptor_set(types).unwrap();
        let error = decode("fixture.empty", pool.get_message_by_name("google.protobuf.Empty").unwrap(), r#"{"extra":true}"#).unwrap_err();
        assert_failure(error, Classification::Invalid, "Call fixture.empty: Field \"extra\" Is Not in Its Request; Expected No Fields");
    }
}
