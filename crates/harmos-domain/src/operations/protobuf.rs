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
use prost::Message;
use prost_reflect::{DescriptorPool, Kind, MessageDescriptor};

// Local Imports
use crate::schema::{Cardinality, graph};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

pub trait ProtobufExt {
    fn pool(&self) -> Result<DescriptorPool, super::ContractError>;
    fn schema(&self) -> Result<graph::Schema, super::ContractError>;
    fn fingerprint(&self, id: &str) -> Result<crate::Fingerprint, super::ContractError>;
}

/* === Implementations === */

impl ProtobufExt for super::Protobuf {
    fn pool(&self) -> Result<DescriptorPool, super::ContractError> {
        DescriptorPool::decode(self.descriptors.as_slice()).map_err(|error| super::ContractError::Protobuf(error.to_string()))
    }

    fn schema(&self) -> Result<graph::Schema, super::ContractError> {
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
            let message = pool.get_message_by_name(message).ok_or_else(|| super::ContractError::Protobuf(message.clone()))?;
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
        let mut definitions = vec![directions];
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
                        let next = messages.len() + 1;
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
        Ok(graph::Schema {
            root: 0,
            definitions,
            representations: BTreeMap::new(),
        })
    }

    fn fingerprint(&self, id: &str) -> Result<crate::Fingerprint, super::ContractError> {
        let descriptors = prost_types::FileDescriptorSet::decode(self.descriptors.as_slice())
            .map_err(|error| super::ContractError::Protobuf(error.to_string()))?;
        let fingerprint = harmos_protocol::schema::fingerprint(id, &descriptors, &self.request, &self.response)
            .map_err(|error| super::ContractError::Protobuf(error.to_string()))?;
        if self.input.is_none() && self.output.is_none() {
            return Ok(crate::Fingerprint(fingerprint.try_into().expect("SHA-256 fingerprint")));
        }
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"harmos.protobuf.stream.v1\0");
        hash.update(fingerprint);
        for message in [&self.input, &self.output] {
            match message {
                None => hash.update([0]),
                Some(message) => {
                    hash.update([1]);
                    let fingerprint = harmos_protocol::schema::fingerprint(id, &descriptors, message, message)
                        .map_err(|error| super::ContractError::Protobuf(error.to_string()))?;
                    hash.update(fingerprint);
                }
            }
        }
        Ok(crate::Fingerprint(hash.finalize().into()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
