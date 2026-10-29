/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub use prost_reflect::{DynamicMessage, MessageDescriptor, ReflectMessage, Value};

use std::borrow::Cow;
use std::collections::HashSet;
use std::mem::size_of;

use prost_reflect::{
    DescriptorPool,
    prost_types::{
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, OneofDescriptorProto,
        field_descriptor_proto::{Label, Type},
    },
};

use harmos_domain::config::SchemaSettings;
use harmos_domain::schema::{Cardinality, graph};

const RESERVED_FIELDS: &[&str] = &[
    "ByteSize",
    "Clear",
    "ClearExtension",
    "ClearField",
    "CopyFrom",
    "DESCRIPTOR",
    "DiscardUnknownFields",
    "Extensions",
    "FindInitializationErrors",
    "FromString",
    "HasExtension",
    "HasField",
    "IsInitialized",
    "ListFields",
    "MergeFrom",
    "MergeFromString",
    "ParseFromString",
    "SerializePartialToString",
    "SerializeToString",
    "SetInParent",
    "UnknownFields",
    "WhichOneof",
];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid protobuf package name")]
    Package,

    #[error(transparent)]
    Schema(#[from] harmos_domain::schema::Error),

    #[error(transparent)]
    Descriptor(#[from] prost_reflect::DescriptorError),
}

pub fn descriptor_schema(schema: &graph::Schema, package: &str, settings: &SchemaSettings) -> Result<MessageDescriptor, Error> {
    descriptor_root(schema, schema.root, package, settings)
}

// Export A Direction Root From A Shared Contract Definition Table.
pub fn descriptor_root(schema: &graph::Schema, root: usize, package: &str, settings: &SchemaSettings) -> Result<MessageDescriptor, Error> {
    if root >= schema.definitions.len() {
        return Err(harmos_domain::schema::Error::Graph(graph::Error::Reference { definition: root }).into());
    }
    if package.len() > settings.preparation_work.get() {
        return Err(harmos_domain::schema::Error::Graph(graph::Error::Budget).into());
    }
    if !package.split('.').all(|part| {
        let mut bytes = part.bytes();
        bytes.next().is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    }) {
        return Err(Error::Package);
    }
    let preparation = schema.prepare(settings).map_err(harmos_domain::schema::Error::Graph)?;
    let retained = descriptor_memory_bytes(schema, package).map_err(harmos_domain::schema::Error::Graph)?;
    if preparation.bytes.checked_add(retained).is_none_or(|bytes| bytes > settings.preparation_bytes.get()) {
        return Err(harmos_domain::schema::Error::Graph(graph::Error::Budget).into());
    }
    let work = schema
        .definitions
        .iter()
        .try_fold(package.len(), |work, fields| {
            fields.iter().try_fold(work, |work, field| {
                field
                    .name
                    .len()
                    .checked_mul(if RESERVED_FIELDS.contains(&field.name.as_ref()) { 3 } else { 2 })
                    .and_then(|bytes| bytes.checked_add(package.len()))
                    .and_then(|bytes| bytes.checked_add(32))
                    .and_then(|bytes| work.checked_add(bytes))
                    .ok_or(graph::Error::Budget)
            })
        })
        .map_err(harmos_domain::schema::Error::Graph)?;
    if preparation.work.checked_add(work).is_none_or(|work| work > settings.preparation_work.get()) {
        return Err(harmos_domain::schema::Error::Graph(graph::Error::Budget).into());
    }
    let messages = schema
        .definitions
        .iter()
        .enumerate()
        .map(|(index, fields)| describe(fields, index, root, package))
        .collect::<Result<Vec<_>, _>>()?;
    let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet {
        file: vec![FileDescriptorProto {
            name: Some(format!("{}/messages.proto", package.replace('.', "/"))),
            package: Some(package.into()),
            syntax: Some("proto3".into()),
            message_type: messages,
            ..Default::default()
        }],
    })?;
    Ok(pool.get_message_by_name(&format!("{package}.Row0")).expect("the root descriptor was inserted"))
}

fn descriptor_memory_bytes(schema: &graph::Schema, package: &str) -> Result<usize, graph::Error> {
    // Account For The Input Protos, Pool Copies, Names And Lookup Tables Before Export.
    let mut bytes = 4usize
        .checked_mul(size_of::<FileDescriptorSet>() + size_of::<FileDescriptorProto>() + package.len() + 32)
        .ok_or(graph::Error::Budget)?;
    let width = schema.definitions.len().to_string().len();
    for fields in &schema.definitions {
        bytes = bytes.checked_add(4 * (size_of::<DescriptorProto>() + package.len() + width + 8)).ok_or(graph::Error::Budget)?;
        for field in fields {
            let names = field
                .name
                .len()
                .checked_add(if RESERVED_FIELDS.contains(&field.name.as_ref()) { field.name.len() + 1 } else { 0 })
                .and_then(|length| length.checked_add(package.len()))
                .and_then(|length| length.checked_add(width + 64))
                .ok_or(graph::Error::Budget)?;
            let field = size_of::<FieldDescriptorProto>()
                .checked_add(size_of::<OneofDescriptorProto>())
                .and_then(|bytes| bytes.checked_add(names))
                .and_then(|bytes| bytes.checked_mul(4))
                .ok_or(graph::Error::Budget)?;
            bytes = bytes.checked_add(field).ok_or(graph::Error::Budget)?;
        }
    }
    Ok(bytes)
}

fn describe(fields: &[graph::Field], index: usize, root: usize, package: &str) -> Result<DescriptorProto, Error> {
    let mut names = HashSet::new();
    for field in fields {
        if !names.insert(field_name(&field.name)) {
            return Err(harmos_domain::schema::Error::Graph(graph::Error::Field {
                field: field.name.to_string(),
                reason: "protobuf field names collide after reserved-member escaping",
            })
            .into());
        }
    }
    let name = row_name(index, root);
    let mut row = DescriptorProto {
        name: Some(name.clone()),
        ..Default::default()
    };
    let variants = fields.iter().any(|field| field.cardinality == Cardinality::Variant);
    if variants {
        let mut variant_name = "_harmos_variant".to_owned();
        while names.contains(variant_name.as_str()) {
            variant_name.push('_');
        }
        row.oneof_decl.push(OneofDescriptorProto {
            name: Some(variant_name),
            ..Default::default()
        });
    }
    for field in fields {
        let (kind, type_name) = match &field.kind {
            graph::Kind::Bool => (Type::Bool, None),
            graph::Kind::U32 => (Type::Uint32, None),
            graph::Kind::U64 => (Type::Uint64, None),
            graph::Kind::I32 => (Type::Int32, None),
            graph::Kind::I64 => (Type::Int64, None),
            graph::Kind::F32 => (Type::Float, None),
            graph::Kind::F64 => (Type::Double, None),
            graph::Kind::String => (Type::String, None),
            graph::Kind::Bytes => (Type::Bytes, None),
            graph::Kind::Message { definition } => (Type::Message, Some(format!(".{package}.{}", row_name(*definition, root)))),
        };
        let optional_scalar = field.cardinality == Cardinality::Optional && kind != Type::Message;
        let oneof_index = if field.cardinality == Cardinality::Variant {
            Some(0)
        } else {
            optional_scalar.then(|| {
                let index = row.oneof_decl.len() as i32;
                // Tags are unique and this name is independent of user field names.
                let mut name = format!("_harmos_presence_{}", field.tag);
                while names.contains(name.as_str()) {
                    name.push('_');
                }
                row.oneof_decl.push(OneofDescriptorProto { name: Some(name), ..Default::default() });
                index
            })
        };
        row.field.push(FieldDescriptorProto {
            name: Some(field_name(&field.name).into_owned()),
            json_name: RESERVED_FIELDS.contains(&field.name.as_ref()).then(|| field.name.to_string()),
            number: Some(field.tag as i32),
            label: Some(if field.cardinality == Cardinality::Repeated { Label::Repeated } else { Label::Optional } as i32),
            r#type: Some(kind as i32),
            type_name,
            oneof_index,
            proto3_optional: optional_scalar.then_some(true),
            ..Default::default()
        });
    }
    Ok(row)
}

fn field_name(name: &str) -> Cow<'_, str> {
    if RESERVED_FIELDS.contains(&name) { Cow::Owned(format!("{name}_")) } else { Cow::Borrowed(name) }
}

fn row_name(definition: usize, root: usize) -> String {
    let index = if definition == root {
        0
    } else if definition < root {
        definition + 1
    } else {
        definition
    };
    format!("Row{index}")
}
