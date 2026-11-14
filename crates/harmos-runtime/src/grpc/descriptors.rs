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
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports
use prost_reflect::{DescriptorPool, MessageDescriptor, ServiceDescriptor};
use prost_types::FileDescriptorSet;

// Local Imports
use super::projection::DescriptorError;
use harmos_operations::operations::Contract;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// Reuse the Projection's Service Envelopes with Authoritative Payload Descriptors.
pub fn descriptor(package: &str, name: &str, methods: &[(&str, &Contract)]) -> Result<ServiceDescriptor, DescriptorError> {
    let projected = super::projection::descriptor(package, name, methods)?;
    let callers = methods.iter().map(|(method, contract)| (format!("{package}.{name}/{method}"), contract.callers())).collect();
    if methods.iter().all(|(_, contract)| contract.protobuf().is_none()) {
        let types = FileDescriptorSet {
            file: projected.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
        };
        let pool = harmos_protocol::schema::with_method_callers(types, &callers)?;
        return Ok(pool.get_service_by_name(projected.full_name()).expect("declared service"));
    }
    let service_file = projected.parent_file().name().to_owned();
    let mut imports = prost_types::FileDescriptorSet::default();
    let mut removed = BTreeSet::new();
    let mut references = BTreeMap::new();
    for (method, contract) in methods {
        let Some(protobuf) = contract.protobuf() else { continue };
        let method = projected.methods().find(|candidate| candidate.name() == *method).expect("projected method");
        let managed = contract.grpc().is_some_and(|grpc| grpc.managed);
        let request = payload(method.input(), "request", managed || contract.input().is_some());
        let response = (managed || contract.output().is_none()).then(|| payload(method.output(), "response", managed));
        let input = contract.input().map(|_| payload(method.input(), "item", true));
        let output = contract.output().map(|_| payload(method.output(), "item", managed));
        for (old, new) in [
            (Some(request), Some(&protobuf.request)),
            (response, Some(&protobuf.response)),
            (input, protobuf.input.as_ref()),
            (output, protobuf.output.as_ref()),
        ] {
            let (Some(old), Some(new)) = (old, new) else { continue };
            removed.insert(old.parent_file().name().to_owned());
            references.insert(format!(".{}", old.full_name()), format!(".{new}"));
        }
        let pool = DescriptorPool::decode(protobuf.descriptors.as_slice())?;
        for file in pool.files() {
            let mut file = file.file_descriptor_proto().clone();
            // The Declaration Owns the Served Methods, Including Visibility.
            file.service.clear();
            harmos_protocol::schema::merge_message_types(&mut imports, prost_types::FileDescriptorSet { file: vec![file] })
                .map_err(|_| DescriptorError::Name)?;
        }
    }
    let mut files = BTreeMap::new();
    for file in projected.parent_pool().files() {
        if removed.contains(file.name()) {
            continue;
        }
        let mut descriptor = file.file_descriptor_proto().clone();
        if file.name() == service_file {
            descriptor.dependency.retain(|name| !removed.contains(name));
            descriptor.dependency.extend(imports.file.iter().filter_map(|file| file.name.clone()));
            descriptor.dependency.sort();
            descriptor.dependency.dedup();
            for service in &mut descriptor.service {
                for method in &mut service.method {
                    for reference in [&mut method.input_type, &mut method.output_type] {
                        if let Some(replacement) = reference.as_ref().and_then(|reference| references.get(reference)) {
                            *reference = Some(replacement.clone());
                        }
                    }
                }
            }
            for message in &mut descriptor.message_type {
                for field in &mut message.field {
                    if let Some(replacement) = field.type_name.as_ref().and_then(|reference| references.get(reference)) {
                        field.type_name = Some(replacement.clone());
                    }
                }
            }
        }
        files.insert(descriptor.name.clone(), descriptor);
    }
    for file in imports.file {
        let name = file.name.clone();
        if let Some(previous) = files.insert(name, file.clone())
            && previous != file
        {
            return Err(DescriptorError::Name);
        }
    }
    let pool = harmos_protocol::schema::with_method_callers(FileDescriptorSet { file: files.into_values().collect() }, &callers)?;
    Ok(pool.get_service_by_name(projected.full_name()).expect("declared service"))
}

/* === Private Functions === */

fn payload(message: MessageDescriptor, field: &str, envelope: bool) -> MessageDescriptor {
    if !envelope {
        return message;
    }
    let field = message.get_field_by_name(field).expect("projected envelope field");
    let prost_reflect::Kind::Message(message) = field.kind() else { unreachable!("message envelope") };
    message
}

/* === Tests === */
