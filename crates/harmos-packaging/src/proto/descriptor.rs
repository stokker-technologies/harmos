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
        DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet, MethodDescriptorProto, OneofDescriptorProto,
        ServiceDescriptorProto,
        field_descriptor_proto::{Label, Type},
    },
};

// Local Imports
use super::reflection;
use harmos_domain::config::SchemaSettings;
use harmos_domain::operations::Contract;

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

pub fn descriptor(package: &str, name: &str, methods: &[(&str, &Contract)]) -> Result<ServiceDescriptor, DescriptorError> {
    if !identifier(name) || !package.split('.').all(identifier) {
        return Err(DescriptorError::Name);
    }
    if name.to_upper_camel_case().is_empty() || package.split('.').any(|part| part.to_snake_case().is_empty()) {
        return Err(DescriptorError::Name);
    }
    let mut names = BTreeSet::new();
    let mut generated = BTreeSet::new();
    let mut operations = BTreeSet::new();
    for (method, contract) in methods {
        if !identifier(method) || !names.insert((*method).to_owned()) {
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
            return Err(DescriptorError::GeneratedName { method: (*method).into() });
        }
        if !operations.insert(contract.operation().id.as_str()) {
            return Err(DescriptorError::DuplicateOperation);
        }
    }
    for (method, _) in methods {
        let managed = format!("Run{method}");
        if !names.insert(managed.clone()) || !generated.insert(managed.to_snake_case()) {
            return Err(DescriptorError::Name);
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
        let root = reflection::descriptor_schema(contract.schema(), &namespace, &SchemaSettings::default())?;
        let messages = root.parent_file_descriptor_proto().clone();
        file.dependency.push(messages.name.clone().expect("projected file is named"));
        files.push(messages);
        let direction = |name: &str| {
            let field = root.get_field_by_name(name).expect("validated contract direction");
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
                envelope(&mut file, &format!("{name}{method}Input"), vec![("request", 1, request.clone()), ("item", 2, item)])
            }
            None => request.clone(),
        };
        let output = match contract.output() {
            Some(_) => {
                let item = direction("output");
                envelope(&mut file, &format!("{name}{method}Output"), vec![("item", 1, item), ("response", 2, response.clone())])
            }
            None => response.clone(),
        };
        service.method.push(MethodDescriptorProto {
            name: Some((*method).into()),
            input_type: Some(input),
            output_type: Some(output),
            client_streaming: Some(contract.input().is_some()),
            server_streaming: Some(contract.output().is_some()),
            ..Default::default()
        });
        let cancellation = format!("{name}{method}Cancellation");
        file.message_type.push(DescriptorProto {
            name: Some(cancellation.clone()),
            ..Default::default()
        });
        let cancellation = format!(".{package}.{cancellation}");
        let mut incoming = vec![("request", 1, request), ("cancel", 3, cancellation.clone()), ("finish", 4, cancellation)];
        if contract.input().is_some() {
            incoming.push(("item", 2, direction("input")));
        }
        let mut outgoing = vec![("response", 2, response)];
        if contract.output().is_some() {
            outgoing.push(("item", 1, direction("output")));
        }
        let input = envelope(&mut file, &format!("{name}{method}RunInput"), incoming);
        let output = envelope(&mut file, &format!("{name}{method}RunOutput"), outgoing);
        service.method.push(MethodDescriptorProto {
            name: Some(format!("Run{method}")),
            input_type: Some(input),
            output_type: Some(output),
            client_streaming: Some(true),
            server_streaming: Some(true),
            ..Default::default()
        });
    }
    file.service.push(service);
    files.push(file);
    let pool = DescriptorPool::from_file_descriptor_set(FileDescriptorSet { file: files })?;
    Ok(pool.get_service_by_name(&format!("{package}.{name}")).expect("validated service was inserted"))
}

/* === Private Functions === */

fn envelope(file: &mut FileDescriptorProto, name: &str, alternatives: Vec<(&str, i32, String)>) -> String {
    let fields = alternatives
        .into_iter()
        .map(|(name, number, type_name)| FieldDescriptorProto {
            name: Some(name.into()),
            number: Some(number),
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
    format!(".{}.{name}", file.package.as_deref().expect("explicit service package"))
}

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/* === Tests === */
