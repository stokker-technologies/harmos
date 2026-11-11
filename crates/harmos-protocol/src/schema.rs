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

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Hash an operation's reachable request and response descriptors, independently
/// of unrelated declarations in the catalog. See the v1 behaviour contract.
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

/// Compute the wire catalog digest without its advertised digest field.
pub fn catalog_digest(catalog: &crate::v1::Catalog) -> Vec<u8> {
    let mut catalog = catalog.clone();
    catalog.digest.clear();
    Sha256::digest(catalog.encode_to_vec()).to_vec()
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
