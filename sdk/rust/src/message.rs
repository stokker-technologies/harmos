/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost::Message as _;
use prost_types::FileDescriptorSet;
use sha2::{Digest, Sha256};

// Local Imports
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/// Protobuf data plus the descriptor needed to advertise its contract.
pub trait Message: prost::Message + Default + 'static {
    fn name() -> String;
    fn types() -> FileDescriptorSet;

    #[doc(hidden)]
    fn register(types: &mut FileDescriptorSet) {
        merge_descriptor_types(types, Self::types());
    }
}

/// A typed dependency generated from, or matched to, the provider's contract.
pub trait OperationRequest: Message {
    type Response: Message;
    const ID: &'static str;
}

/* === Implementations === */

impl Message for () {
    fn name() -> String {
        "google.protobuf.Empty".into()
    }
    fn types() -> FileDescriptorSet {
        FileDescriptorSet {
            file: vec![prost_types::FileDescriptorProto {
                name: Some("google/protobuf/empty.proto".into()),
                package: Some("google.protobuf".into()),
                syntax: Some("proto3".into()),
                message_type: vec![prost_types::DescriptorProto {
                    name: Some("Empty".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }
}

/* === Public Functions === */

pub fn fingerprint<Q: Message, R: Message>(id: &str) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(id.as_bytes());
    hash.update([0]);
    hash.update(Q::types().encode_to_vec());
    hash.update(R::types().encode_to_vec());
    hash.finalize().to_vec()
}

/* === Private Functions === */

pub fn merge_descriptor_types(target: &mut FileDescriptorSet, source: FileDescriptorSet) {
    for mut file in source.file {
        if let Some(existing) = target
            .file
            .iter_mut()
            .find(|existing| existing.name == file.name && existing.package == file.package && existing.syntax == file.syntax)
        {
            for dependency in file.dependency.drain(..) {
                if !existing.dependency.contains(&dependency) {
                    existing.dependency.push(dependency);
                }
            }
            for message in file.message_type.drain(..) {
                if !existing.message_type.contains(&message) {
                    existing.message_type.push(message);
                }
            }
        } else {
            target.file.push(file);
        }
    }
}

pub(crate) fn merge_types(target: &mut FileDescriptorSet, source: FileDescriptorSet) -> Result<()> {
    merge_descriptor_types(target, source);
    let mut files = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for file in &target.file {
        if !files.insert(&file.name) {
            return Err(Error::Invalid(format!("Conflicting Protobuf File: {:?}", file.name)));
        }
        for message in &file.message_type {
            if !names.insert((&file.package, &message.name)) {
                return Err(Error::Invalid(format!("Conflicting Protobuf Message: {:?}", message.name)));
            }
        }
    }
    Ok(())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(crate::Message)]
    struct Reading {
        celsius: f64,
    }

    #[derive(crate::Message)]
    struct Data {
        text: String,
        bytes: Vec<u8>,
        count: Option<i32>,
        readings: Vec<Reading>,
        present: Option<Reading>,
    }

    #[derive(crate::Message)]
    struct Node {
        children: Vec<Node>,
    }

    #[test]
    fn derived_messages_round_trip_protobuf_fields() {
        let value = Data {
            text: "reading".into(),
            bytes: vec![0, 255],
            count: Some(0),
            readings: vec![Reading { celsius: -12.5 }],
            present: Some(Reading::default()),
        };
        let bytes = value.encode_to_vec();
        let decoded = Data::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.text, value.text);
        assert_eq!(decoded.bytes, value.bytes);
        assert_eq!(decoded.count, Some(0));
        assert_eq!(decoded.readings[0].celsius, -12.5);
        assert!(decoded.present.is_some());
        assert_eq!(decoded.encoded_len(), bytes.len());
        let mut decoded = decoded;
        decoded.clear();
        assert!(decoded.encode_to_vec().is_empty());
    }

    #[test]
    fn descriptors_resolve_nested_and_optional_fields() {
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(Data::types()).unwrap();
        let descriptor = pool.get_message_by_name(&Data::name()).unwrap();
        let value = Data {
            count: Some(0),
            readings: vec![Reading { celsius: 1.5 }],
            ..Default::default()
        };
        let decoded = prost_reflect::DynamicMessage::decode(descriptor, value.encode_to_vec().as_slice()).unwrap();
        assert!(decoded.has_field_by_name("count"));
        assert!(!decoded.has_field_by_name("present"));
        assert_eq!(decoded.encode_to_vec(), value.encode_to_vec());
    }

    #[test]
    fn recursive_message_descriptors_terminate_and_resolve() {
        let pool = prost_reflect::DescriptorPool::from_file_descriptor_set(Node::types()).unwrap();
        let value = Node { children: vec![Node::default()] };
        let descriptor = pool.get_message_by_name(&Node::name()).unwrap();
        let decoded = prost_reflect::DynamicMessage::decode(descriptor, value.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded.encode_to_vec(), value.encode_to_vec());
    }
}
