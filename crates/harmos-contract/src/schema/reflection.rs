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
pub use prost_reflect::{DynamicMessage, MessageDescriptor, ReflectMessage, Value};

/* === Imports === */
// Standard Library Imports

// Third Party Imports
use prost_reflect::{
    DescriptorPool,
    prost_types::{
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
        OneofDescriptorProto,
        field_descriptor_proto::{Label, Type},
    },
};
// Local Imports
use super::{Cardinality, Field, Kind};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid protobuf package name")]
    Package,
    #[error(transparent)]
    Schema(#[from] super::Error),
    #[error(transparent)]
    Descriptor(#[from] prost_reflect::DescriptorError),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn descriptor(fields: &[Field]) -> Result<MessageDescriptor, Error> {
    descriptor_in(fields, "harmos.stream")
}

pub fn descriptor_in(fields: &[Field], package: &str) -> Result<MessageDescriptor, Error> {
    if !package.split('.').all(|part| {
        let mut bytes = part.bytes();
        bytes
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    }) {
        return Err(Error::Package);
    }
    super::validate(fields)?;
    let mut messages = Vec::new();
    describe(fields, &mut messages, package);
    let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet {
        file: vec![FileDescriptorProto {
            name: Some(format!("{}/messages.proto", package.replace('.', "/"))),
            package: Some(package.into()),
            syntax: Some("proto3".into()),
            message_type: messages,
            ..Default::default()
        }],
    })?;
    Ok(pool
        .get_message_by_name(&format!("{package}.Row0"))
        .expect("the root descriptor was inserted"))
}

/* === Private Functions === */
fn describe(fields: &[Field], messages: &mut Vec<DescriptorProto>, package: &str) -> String {
    let index = messages.len();
    let name = format!("Row{index}");
    messages.push(DescriptorProto::default());
    let mut row = DescriptorProto {
        name: Some(name.clone()),
        ..Default::default()
    };
    let variants = fields
        .iter()
        .any(|field| field.cardinality == Cardinality::Variant);
    if variants {
        let mut variant_name = "_harmos_variant".to_owned();
        while fields.iter().any(|field| field.name == variant_name) {
            variant_name.push('_');
        }
        row.oneof_decl.push(OneofDescriptorProto {
            name: Some(variant_name),
            ..Default::default()
        });
    }
    for field in fields {
        let (kind, type_name) = match &field.kind {
            Kind::Bool => (Type::Bool, None),
            Kind::U32 => (Type::Uint32, None),
            Kind::U64 => (Type::Uint64, None),
            Kind::I32 => (Type::Int32, None),
            Kind::I64 => (Type::Int64, None),
            Kind::F32 => (Type::Float, None),
            Kind::F64 => (Type::Double, None),
            Kind::String => (Type::String, None),
            Kind::Bytes => (Type::Bytes, None),
            Kind::Message { fields } => (Type::Message, Some(describe(fields, messages, package))),
        };
        let optional_scalar = field.cardinality == Cardinality::Optional && kind != Type::Message;
        let oneof_index = if field.cardinality == Cardinality::Variant {
            Some(0)
        } else {
            optional_scalar.then(|| {
                let index = row.oneof_decl.len() as i32;
                // Tags are unique and this name is independent of user field names.
                let mut name = format!("_harmos_presence_{}", field.tag);
                while fields.iter().any(|field| field.name == name) {
                    name.push('_');
                }
                row.oneof_decl.push(OneofDescriptorProto {
                    name: Some(name),
                    ..Default::default()
                });
                index
            })
        };
        row.field.push(FieldDescriptorProto {
            name: Some(field.name.to_string()),
            number: Some(field.tag as i32),
            label: Some(if field.cardinality == Cardinality::Repeated {
                Label::Repeated
            } else {
                Label::Optional
            } as i32),
            r#type: Some(kind as i32),
            type_name,
            oneof_index,
            proto3_optional: optional_scalar.then_some(true),
            ..Default::default()
        });
    }
    messages[index] = row;
    format!(".{package}.{name}")
}

/* === Tests === */
