/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Descriptor Fingerprints

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use crate::model::Declarations;

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports
use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorSet};
use sha2::{Digest, Sha256};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Debug, thiserror::Error)]
#[error("Invalid Protobuf Schema: {0}")]
pub struct Error(String);

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationFace {
    Plugin,
    Context,
    Client,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Validate a Wire Operation Identity and Resolve Its Audience Face.
pub fn operation_identity(id: &str) -> Result<OperationFace, Error> {
    let invalid = || Error(format!("Declare {id}: Invalid Wire Operation Identity"));
    let Some((service, method)) = id.split_once('/') else {
        if id.split('.').all(|segment| !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')) {
            return Ok(OperationFace::Plugin);
        }
        return Err(invalid());
    };
    let (package, service) = service.rsplit_once('.').ok_or_else(invalid)?;
    let identifier = |value: &str| {
        let mut bytes = value.bytes();
        bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    };
    if !package.split('.').all(identifier) || !identifier(service) || !identifier(method) {
        return Err(invalid());
    }
    let segments: Vec<_> = package.split('.').collect();
    let mut face = OperationFace::Client;
    for (index, segment) in segments.iter().enumerate().skip(1) {
        let version =
            segment.strip_prefix('v').is_some_and(|version| !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit()));
        if !version {
            continue;
        }
        let boundary = if index >= 2 && segments[index - 2..index] == ["plugin", "host"] {
            Some(OperationFace::Context)
        } else if segments[index - 1] == "plugin" {
            Some(OperationFace::Plugin)
        } else {
            None
        };
        if let Some(boundary) = boundary {
            if face != OperationFace::Client {
                return Err(invalid());
            }
            face = boundary;
        }
    }
    Ok(face)
}

/// Admit Only Plugin-Face Identities Outside the Library Namespace.
pub fn plugin_operation_identity(id: &str) -> Result<(), Error> {
    if operation_identity(id)? != OperationFace::Plugin {
        return Err(Error(format!("Register {id}: Serve Operations on the Plugin Face")));
    }
    if id.starts_with("harmos.") || id.starts_with("harmos_") {
        return Err(Error(format!("Register {id}: Host Namespace Is Reserved")));
    }
    Ok(())
}

/// Hash an operation's reachable request and response descriptors, independently
/// of unrelated declarations in the declarations. See the v1 behaviour contract.
pub fn fingerprint(id: &str, types: &FileDescriptorSet, request: &str, response: &str) -> Result<Vec<u8>, Error> {
    let request = message_types(types, request)?;
    let response = message_types(types, response)?;
    let mut hash = Sha256::new();
    hash.update(id.as_bytes());
    hash.update([0]);
    hash.update(request.encode_to_vec());
    hash.update(response.encode_to_vec());
    Ok(hash.finalize().to_vec())
}

/// Fingerprint A Host Operation, Including Its Optional Stream Message Graphs.
pub fn host_fingerprint(id: &str, contract: &crate::model::Protobuf) -> Result<Vec<u8>, Error> {
    let descriptors = FileDescriptorSet::decode(contract.descriptors.as_slice()).map_err(|error| Error(error.to_string()))?;
    let fingerprint = fingerprint(id, &descriptors, &contract.request, &contract.response)?;
    if contract.input.is_none() && contract.output.is_none() {
        return Ok(fingerprint);
    }
    let mut hash = Sha256::new();
    hash.update(b"harmos.protobuf.stream.v1\0");
    hash.update(fingerprint);
    for message in [&contract.input, &contract.output] {
        match message {
            None => hash.update([0]),
            Some(message) => {
                hash.update([1]);
                hash.update(self::fingerprint(id, &descriptors, message, message)?);
            }
        }
    }
    Ok(hash.finalize().to_vec())
}

/// Compute the wire declarations digest without its advertised digest field.
pub fn declarations_digest(declarations: &Declarations) -> Vec<u8> {
    let mut declarations = declarations.clone();
    declarations.digest.clear();
    Sha256::digest(declarations.encode_to_vec()).to_vec()
}

/// Preprojected Message Options for Validation Without Loading Unrelated Meta-Schemas.
pub fn descriptor_options() -> FileDescriptorSet {
    FileDescriptorSet::decode(include_bytes!("message-options-descriptors.bin").as_slice()).expect("generated message option descriptors")
}

/// Project a message's descriptor graph into the canonical data-only set.
pub fn message_types(types: &FileDescriptorSet, root: &str) -> Result<FileDescriptorSet, Error> {
    let mut symbols = BTreeMap::new();
    let mut messages = BTreeSet::new();
    let mut files = BTreeSet::new();
    for (file_index, file) in types.file.iter().enumerate() {
        let name = file.name.as_ref().ok_or_else(|| Error("Missing File Name".into()))?;
        if !files.insert(name) {
            return Err(Error(format!("Duplicate File: {name}")));
        }
        let package = file.package.as_deref().unwrap_or_default();
        for (index, message) in file.message_type.iter().enumerate() {
            index_message(message, package, (file_index, false, index), &mut symbols, &mut messages)?;
        }
        for (index, enumeration) in file.enum_type.iter().enumerate() {
            let name = qualified(package, enumeration.name.as_deref())?;
            if symbols.insert(name.clone(), (file_index, true, index)).is_some() {
                return Err(Error(format!("Duplicate Type: {name}")));
            }
        }
    }
    let root = root.strip_prefix('.').unwrap_or(root);
    if !messages.contains(root) {
        return Err(Error(format!("Missing Message: {root}")));
    }

    let mut pending = vec![root.to_owned()];
    let mut selected = BTreeSet::new();
    while let Some(name) = pending.pop() {
        let symbol = name.strip_prefix('.').unwrap_or(&name);
        let owner = *symbols.get(symbol).ok_or_else(|| Error(format!("Missing Type: {name}")))?;
        if !selected.insert(owner) || owner.1 {
            continue;
        }
        references(&types.file[owner.0].message_type[owner.2], &mut pending)?;
    }

    let mut result = FileDescriptorSet::default();
    for (file_index, original) in types.file.iter().enumerate() {
        let mut file = original.clone();
        file.message_type = original
            .message_type
            .iter()
            .enumerate()
            .filter(|(index, _)| selected.contains(&(file_index, false, *index)))
            .map(|(_, message)| message.clone())
            .collect();
        file.enum_type = original
            .enum_type
            .iter()
            .enumerate()
            .filter(|(index, _)| selected.contains(&(file_index, true, *index)))
            .map(|(_, enumeration)| enumeration.clone())
            .collect();
        if file.message_type.is_empty() && file.enum_type.is_empty() {
            continue;
        }
        file.message_type.sort_by(|left, right| left.name.cmp(&right.name));
        file.enum_type.sort_by(|left, right| left.name.cmp(&right.name));
        file.source_code_info = None;
        file.options = None;
        file.service.clear();
        file.extension.clear();
        file.public_dependency.clear();
        file.weak_dependency.clear();
        for message in &mut file.message_type {
            normalize_message(message);
        }
        result.file.push(file);
    }
    let selected_files: BTreeSet<_> = result.file.iter().filter_map(|file| file.name.clone()).collect();
    for file in &mut result.file {
        file.dependency.retain(|name| selected_files.contains(name));
        file.dependency.sort();
        file.dependency.dedup();
    }
    result.file.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(result)
}

/// Retain Exactly the Descriptor Graphs Reachable from These Message Roots.
pub fn operation_types(types: &FileDescriptorSet, roots: &[&str]) -> Result<FileDescriptorSet, Error> {
    let mut result = FileDescriptorSet::default();
    for root in roots {
        merge_message_types(&mut result, message_types(types, root)?)?;
    }
    Ok(result)
}

/// Merge Data-Only Descriptor Graphs, Refusing Conflicting Definitions.
pub fn merge_message_types(target: &mut FileDescriptorSet, source: FileDescriptorSet) -> Result<(), Error> {
    for mut file in source.file {
        let name = file.name.as_ref().ok_or_else(|| Error("Missing File Name".into()))?;
        if !file.service.is_empty() || !file.extension.is_empty() {
            return Err(Error("Expected Data-Only Descriptors".into()));
        }
        let Some(existing) = target.file.iter_mut().find(|existing| existing.name.as_ref() == Some(name)) else {
            target.file.push(file);
            continue;
        };
        if existing.package != file.package || existing.syntax != file.syntax || existing.options != file.options {
            return Err(Error(format!("Conflicting File: {name}")));
        }
        for message in file.message_type.drain(..) {
            if let Some(previous) = existing.message_type.iter().find(|previous| previous.name == message.name) {
                if previous != &message {
                    return Err(Error(format!("Conflicting Message: {name}")));
                }
            } else {
                existing.message_type.push(message);
            }
        }
        for enumeration in file.enum_type.drain(..) {
            if let Some(previous) = existing.enum_type.iter().find(|previous| previous.name == enumeration.name) {
                if previous != &enumeration {
                    return Err(Error(format!("Conflicting Enum: {name}")));
                }
            } else {
                existing.enum_type.push(enumeration);
            }
        }
        existing.message_type.sort_by(|left, right| left.name.cmp(&right.name));
        existing.enum_type.sort_by(|left, right| left.name.cmp(&right.name));
        existing.dependency.append(&mut file.dependency);
        existing.dependency.sort();
        existing.dependency.dedup();
    }
    target.file.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(())
}

/* === Private Functions === */

fn qualified(parent: &str, name: Option<&str>) -> Result<String, Error> {
    let name = name.filter(|name| !name.is_empty()).ok_or_else(|| Error("Missing Type Name".into()))?;
    Ok(if parent.is_empty() { name.to_owned() } else { format!("{parent}.{name}") })
}

fn index_message(
    message: &DescriptorProto, parent: &str, owner: (usize, bool, usize), symbols: &mut BTreeMap<String, (usize, bool, usize)>,
    messages: &mut BTreeSet<String>,
) -> Result<(), Error> {
    let name = qualified(parent, message.name.as_deref())?;
    if symbols.insert(name.clone(), owner).is_some() {
        return Err(Error(format!("Duplicate Type: {name}")));
    }
    messages.insert(name.clone());
    for child in &message.nested_type {
        index_message(child, &name, owner, symbols, messages)?;
    }
    for enumeration in &message.enum_type {
        let name = qualified(&name, enumeration.name.as_deref())?;
        if symbols.insert(name.clone(), owner).is_some() {
            return Err(Error(format!("Duplicate Type: {name}")));
        }
    }
    Ok(())
}

fn references(message: &DescriptorProto, pending: &mut Vec<String>) -> Result<(), Error> {
    for field in message.field.iter().chain(&message.extension) {
        for name in [&field.type_name, &field.extendee].into_iter().flatten() {
            if !name.starts_with('.') {
                return Err(Error(format!("Type Reference Must Be Fully Qualified: {name}")));
            }
            pending.push(name.clone());
        }
    }
    for child in &message.nested_type {
        references(child, pending)?;
    }
    Ok(())
}

fn normalize_message(message: &mut DescriptorProto) {
    for field in message.field.iter_mut().chain(&mut message.extension) {
        field.json_name = None;
    }
    for child in &mut message.nested_type {
        normalize_message(child);
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_identities_resolve_faces_without_native_segment_restrictions() {
        for (id, face) in [
            ("example.step.0", OperationFace::Plugin),
            ("example.criterion.12", OperationFace::Plugin),
            ("example.plugin.v1.Lifecycle/Initialize", OperationFace::Plugin),
            ("example.plugin.v2.steps.Progress/Get", OperationFace::Plugin),
            ("example.plugin.host.v1.Prompts/Create", OperationFace::Context),
            ("example.plugin.host.v1.jobs.Progress/Watch", OperationFace::Context),
            ("example.v1.Steps/Execute", OperationFace::Client),
            ("example.v1.jobs.Progress/Get", OperationFace::Client),
            ("example.context.values.Prompts/Create", OperationFace::Client),
            ("_example.v1._Service/_Method", OperationFace::Client),
        ] {
            assert_eq!(operation_identity(id).unwrap(), face, "{id}");
            assert_eq!(plugin_operation_identity(id).is_ok(), face == OperationFace::Plugin, "{id}");
        }
        for id in [
            "",
            ".example",
            "example.",
            "example..step.0",
            "example step",
            "example.-step",
            "example.écho",
            "Steps/Execute",
            ".Steps/Execute",
            "example.plugin.v1.Steps/",
            "example.plugin.v1./Execute",
            "example.plugin.v1.Steps/Execute/Again",
            "example.0.Steps/Execute",
            "example.v1.Steps/0",
            "example.plugin.v1.plugin.host.v1.Steps/Execute",
        ] {
            let error = operation_identity(id).unwrap_err().to_string();
            assert!(error.contains("Declare"), "{error}");
            assert!(error.contains(id), "{error}");
        }
        for id in
            ["harmos.plugin.v1.Instance/Invoke", "harmos.plugin.host.v1.Time/Get", "harmos.client.v1.Time/Get", "harmos_forged.step.0"]
        {
            assert!(operation_identity(id).is_ok());
            assert!(plugin_operation_identity(id).unwrap_err().to_string().contains(id));
        }
    }
}
