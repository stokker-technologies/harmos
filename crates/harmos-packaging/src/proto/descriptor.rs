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
use std::collections::BTreeSet;

// Third Party Imports
use heck::{ToSnakeCase, ToUpperCamelCase};
use prost_reflect::{
    DescriptorPool, ServiceDescriptor,
    prost_types::{
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
        MethodDescriptorProto, OneofDescriptorProto, ServiceDescriptorProto,
        field_descriptor_proto::{Label, Type},
    },
};

// Local Imports
use harmos_domain::config::SchemaSettings;
use harmos_domain::operations::Contract;
use super::reflection;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum DescriptorError {
    #[error("invalid or duplicate gRPC method/service name")]
    Name,
    #[error("gRPC method `{method}` collides with a generated Rust name")]
    GeneratedName { method: String },
    #[error("operation identity appears more than once in the service")]
    DuplicateOperation,
    #[error(transparent)]
    Schema(#[from] reflection::Error),
    #[error(transparent)]
    Descriptor(#[from] prost_reflect::DescriptorError),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn descriptor(
    package: &str,
    name: &str,
    methods: &[(&str, &Contract)],
) -> Result<ServiceDescriptor, DescriptorError> {
    if !identifier(name) || !package.split('.').all(identifier) {
        return Err(DescriptorError::Name);
    }
    if name.to_upper_camel_case().is_empty()
        || package
            .split('.')
            .any(|part| part.to_snake_case().is_empty())
    {
        return Err(DescriptorError::Name);
    }
    let mut names = BTreeSet::new();
    let mut generated = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for (method, contract) in methods {
        if !identifier(method) || !names.insert(*method) {
            return Err(DescriptorError::Name);
        }
        let rust_name = method.to_snake_case();
        if rust_name.is_empty()
            || matches!(
                rust_name.as_str(),
                "new"
                    | "connect"
                    | "with_origin"
                    | "with_interceptor"
                    | "send_compressed"
                    | "accept_compressed"
                    | "max_decoding_message_size"
                    | "max_encoding_message_size"
            )
            || !generated.insert(rust_name)
        {
            return Err(DescriptorError::GeneratedName {
                method: (*method).into(),
            });
        }
        if !operations.insert(contract.operation().id.as_str()) {
            return Err(DescriptorError::DuplicateOperation);
        }
    }
    let mut files = Vec::new();
    let mut service = ServiceDescriptorProto {
        name: Some(name.into()),
        ..Default::default()
    };
    let mut file = FileDescriptorProto {
        name: Some(format!("{}/{name}.proto", package.replace('.', "/"))),
        package: Some(package.into()),
        syntax: Some("proto3".into()),
        ..Default::default()
    };
    for (method, contract) in methods {
        let namespace = format!("{package}.{name}Messages.{method}");
        let root = reflection::descriptor_schema(
            contract.schema(),
            &namespace,
            &SchemaSettings::default(),
        )?;
        let messages = root.parent_file_descriptor_proto().clone();
        file.dependency
            .push(messages.name.clone().expect("projected file is named"));
        files.push(messages);
        let direction = |name: &str| {
            let field = root
                .get_field_by_name(name)
                .expect("validated contract direction");
            let prost_reflect::Kind::Message(message) = field.kind() else {
                unreachable!("contract directions are message references");
            };
            format!(".{}", message.full_name())
        };
        let request = direction("request");
        let response = direction("response");
        let input = match contract.input() {
            Some(_) => {
                let item = direction("input");
                envelope(
                    &mut file,
                    &format!("{name}{method}Input"),
                    [("request", request), ("item", item)],
                )
            }
            None => request,
        };
        let output = match contract.output() {
            Some(_) => {
                let item = direction("output");
                envelope(
                    &mut file,
                    &format!("{name}{method}Output"),
                    [("item", item), ("response", response)],
                )
            }
            None => response,
        };
        service.method.push(MethodDescriptorProto {
            name: Some((*method).into()),
            input_type: Some(input),
            output_type: Some(output),
            client_streaming: Some(contract.input().is_some()),
            server_streaming: Some(contract.output().is_some()),
            ..Default::default()
        });
    }
    file.service.push(service);
    files.push(file);
    let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet { file: files })?;
    Ok(pool
        .get_service_by_name(&format!("{package}.{name}"))
        .expect("validated service was inserted"))
}

/* === Private Functions === */
fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn envelope(
    file: &mut FileDescriptorProto,
    name: &str,
    alternatives: [(&str, String); 2],
) -> String {
    let fields = alternatives
        .into_iter()
        .enumerate()
        .map(|(index, (name, type_name))| FieldDescriptorProto {
            name: Some(name.into()),
            number: Some(index as i32 + 1),
            label: Some(Label::Optional as i32),
            r#type: Some(Type::Message as i32),
            type_name: Some(type_name),
            oneof_index: Some(0),
            ..Default::default()
        })
        .collect();
    file.message_type.push(DescriptorProto {
        name: Some(name.into()),
        field: fields,
        oneof_decl: vec![OneofDescriptorProto {
            name: Some("value".into()),
            ..Default::default()
        }],
        ..Default::default()
    });
    format!(
        ".{}.{name}",
        file.package
            .as_deref()
            .expect("service package is explicit")
    )
}

/* === Tests === */
