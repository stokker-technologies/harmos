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
use std::collections::BTreeMap;

// Third Party Imports
use prost_reflect::{DescriptorPool, Kind, MessageDescriptor};

// Local Imports
use super::{ContractError, Protobuf};
use harmos_domain::Fingerprint;
use harmos_domain::schema::{Cardinality, graph};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait ProtobufExt {
    fn pool(&self) -> Result<DescriptorPool, ContractError>;
    fn schema(&self) -> Result<graph::Schema, ContractError>;
    fn fingerprint(&self, id: &str) -> Result<Fingerprint, ContractError>;
}

/* === Implementations === */

impl ProtobufExt for Protobuf {
    fn pool(&self) -> Result<DescriptorPool, ContractError> {
        DescriptorPool::decode(self.descriptors.as_slice()).map_err(|error| ContractError::Protobuf(error.to_string()))
    }

    fn schema(&self) -> Result<graph::Schema, ContractError> {
        let pool = self.pool()?;
        let mut messages = Vec::<MessageDescriptor>::new();
        let mut indices = BTreeMap::new();
        let mut directions = Vec::new();
        for (name, message) in [
            ("request", Some(&self.request)),
            ("response", Some(&self.response)),
            ("input", self.input.as_ref()),
            ("output", self.output.as_ref()),
        ] {
            let Some(message) = message else { continue };
            let message = pool.get_message_by_name(message).ok_or_else(|| ContractError::Protobuf(message.clone()))?;
            let next = messages.len() + 1;
            let index = *indices.entry(message.full_name().to_owned()).or_insert_with(|| {
                messages.push(message);
                next
            });
            directions.push(graph::Field {
                name: name.into(),
                tag: directions.len() as u32 + 1,
                kind: graph::Kind::Message { definition: index },
                cardinality: Cardinality::Required,
            });
        }
        Ok(message_schema(messages, vec![directions]))
    }

    fn fingerprint(&self, id: &str) -> Result<Fingerprint, ContractError> {
        let fingerprint =
            harmos_protocol::schema::host_fingerprint(id, self).map_err(|error| ContractError::Protobuf(error.to_string()))?;
        Ok(Fingerprint(fingerprint.try_into().expect("SHA-256 fingerprint")))
    }
}

/* === Public Functions === */

pub(crate) fn message_schema(mut messages: Vec<MessageDescriptor>, mut definitions: Vec<Vec<graph::Field>>) -> graph::Schema {
    let offset = definitions.len();
    let mut indices =
        messages.iter().enumerate().map(|(index, message)| (message.full_name().to_owned(), index + offset)).collect::<BTreeMap<_, _>>();
    let mut cursor = 0;
    while cursor < messages.len() {
        let message = messages[cursor].clone();
        let mut fields = Vec::new();
        for field in message.fields() {
            let kind = match field.kind() {
                Kind::Bool => graph::Kind::Bool,
                Kind::Uint32 | Kind::Fixed32 => graph::Kind::U32,
                Kind::Uint64 | Kind::Fixed64 => graph::Kind::U64,
                Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 | Kind::Enum(_) => graph::Kind::I32,
                Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => graph::Kind::I64,
                Kind::Float => graph::Kind::F32,
                Kind::Double => graph::Kind::F64,
                Kind::String => graph::Kind::String,
                Kind::Bytes => graph::Kind::Bytes,
                Kind::Message(message) => {
                    let next = messages.len() + offset;
                    let index = *indices.entry(message.full_name().to_owned()).or_insert_with(|| {
                        messages.push(message);
                        next
                    });
                    graph::Kind::Message { definition: index }
                }
            };
            fields.push(graph::Field {
                name: field.name().to_owned().into(),
                tag: field.number(),
                kind,
                cardinality: if field.is_list() || field.is_map() {
                    Cardinality::Repeated
                } else if field.supports_presence() {
                    Cardinality::Optional
                } else {
                    Cardinality::Required
                },
            });
        }
        fields.sort_by_key(|field| field.tag);
        definitions.push(fields);
        cursor += 1;
    }
    graph::Schema {
        root: 0,
        definitions,
        representations: BTreeMap::new(),
    }
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use prost_types::{DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, field_descriptor_proto};

    #[test]
    fn direction_roots_share_recursive_message_definitions() {
        let descriptors = FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("recursive.proto".into()),
                package: Some("codec".into()),
                syntax: Some("proto3".into()),
                message_type: vec![DescriptorProto {
                    name: Some("Node".into()),
                    field: vec![FieldDescriptorProto {
                        name: Some("next".into()),
                        number: Some(3),
                        r#type: Some(field_descriptor_proto::Type::Message as i32),
                        type_name: Some(".codec.Node".into()),
                        label: Some(field_descriptor_proto::Label::Optional as i32),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        let protobuf = Protobuf {
            descriptors: descriptors.encode_to_vec(),
            request: "codec.Node".into(),
            response: "codec.Node".into(),
            input: Some("codec.Node".into()),
            output: Some("codec.Node".into()),
        };
        let schema = protobuf.schema().unwrap();
        schema.prepare(&Default::default()).unwrap();
        assert_eq!(schema.definitions.len(), 2);
        for (field, name) in schema.definitions[0].iter().zip(["request", "response", "input", "output"]) {
            assert_eq!(field.name, name);
            assert_eq!(field.kind, graph::Kind::Message { definition: 1 });
        }
        assert_eq!(schema.definitions[0].len(), 4);
        assert_eq!(schema.definitions[1][0].kind, graph::Kind::Message { definition: 1 });
    }
}
