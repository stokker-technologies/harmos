/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Canonical Reachable Message Graphs and Conflict-Free Merging.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports
use prost_types::{DescriptorProto, FileDescriptorSet};

// Local Imports
use super::Error;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Nested Types Select Their Top-Level Owner, Preserving Protobuf Scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Owner {
    file: usize,
    declaration: Declaration,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Declaration {
    Message(usize),
    Enum(usize),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Project a message's descriptor graph into the canonical data-only set.
pub fn message_types(types: &FileDescriptorSet, root: &str) -> Result<FileDescriptorSet, Error> {
    let mut symbols = BTreeMap::new();
    let mut messages = BTreeSet::new();
    let mut files = BTreeSet::new();
    for (file_index, file) in types.file.iter().enumerate() {
        let name = file.name.as_ref().ok_or_else(|| Error("Project Message Types: Name Every Descriptor File".into()))?;
        if !files.insert(name) {
            return Err(Error(format!("Project Message Types: Declare File {name} Only Once")));
        }
        let package = file.package.as_deref().unwrap_or_default();
        for (index, message) in file.message_type.iter().enumerate() {
            let owner = Owner {
                file: file_index,
                declaration: Declaration::Message(index),
            };
            index_message(message, package, owner, &mut symbols, &mut messages)?;
        }
        for (index, enumeration) in file.enum_type.iter().enumerate() {
            let name = qualified(package, enumeration.name.as_deref())?;
            let owner = Owner {
                file: file_index,
                declaration: Declaration::Enum(index),
            };
            if symbols.insert(name.clone(), owner).is_some() {
                return Err(Error(format!("Project Message Types: Declare Type {name} Only Once")));
            }
        }
    }
    let root = root.strip_prefix('.').unwrap_or(root);
    if !messages.contains(root) {
        return Err(Error(format!("Project Message Types: Declare Root Message {root}")));
    }

    let mut pending = vec![root.to_owned()];
    let mut selected = BTreeSet::new();
    while let Some(name) = pending.pop() {
        let symbol = name.strip_prefix('.').unwrap_or(&name);
        let owner = *symbols.get(symbol).ok_or_else(|| Error(format!("Project Message Types: Include Referenced Type {name}")))?;
        if !selected.insert(owner) {
            continue;
        }
        if let Declaration::Message(index) = owner.declaration {
            references(&types.file[owner.file].message_type[index], &mut pending)?;
        }
    }

    let mut result = FileDescriptorSet::default();
    for (file_index, original) in types.file.iter().enumerate() {
        let mut file = original.clone();
        file.message_type = original
            .message_type
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                selected.contains(&Owner {
                    file: file_index,
                    declaration: Declaration::Message(*index),
                })
            })
            .map(|(_, message)| message.clone())
            .collect();
        file.enum_type = original
            .enum_type
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                selected.contains(&Owner {
                    file: file_index,
                    declaration: Declaration::Enum(*index),
                })
            })
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
        let name = file.name.as_ref().ok_or_else(|| Error("Project Message Types: Name Every Descriptor File".into()))?;
        if !file.service.is_empty() || !file.extension.is_empty() {
            return Err(Error("Merge Message Types: Remove Services and File Extensions Before Merging".into()));
        }
        let Some(existing) = target.file.iter_mut().find(|existing| existing.name.as_ref() == Some(name)) else {
            target.file.push(file);
            continue;
        };
        if existing.package != file.package || existing.syntax != file.syntax || existing.options != file.options {
            return Err(Error(format!("Merge Message Types: Supply One Consistent Definition of File {name}")));
        }
        for message in file.message_type.drain(..) {
            if let Some(previous) = existing.message_type.iter().find(|previous| previous.name == message.name) {
                if previous != &message {
                    return Err(Error(format!("Merge Message Types: Supply One Consistent Message Definition in {name}")));
                }
            } else {
                existing.message_type.push(message);
            }
        }
        for enumeration in file.enum_type.drain(..) {
            if let Some(previous) = existing.enum_type.iter().find(|previous| previous.name == enumeration.name) {
                if previous != &enumeration {
                    return Err(Error(format!("Merge Message Types: Supply One Consistent Enum Definition in {name}")));
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
    let name = name.filter(|name| !name.is_empty()).ok_or_else(|| Error("Project Message Types: Name Every Type".into()))?;
    Ok(if parent.is_empty() { name.to_owned() } else { format!("{parent}.{name}") })
}

fn index_message(
    message: &DescriptorProto, parent: &str, owner: Owner, symbols: &mut BTreeMap<String, Owner>, messages: &mut BTreeSet<String>,
) -> Result<(), Error> {
    let name = qualified(parent, message.name.as_deref())?;
    if symbols.insert(name.clone(), owner).is_some() {
        return Err(Error(format!("Project Message Types: Declare Type {name} Only Once")));
    }
    messages.insert(name.clone());
    for child in &message.nested_type {
        index_message(child, &name, owner, symbols, messages)?;
    }
    for enumeration in &message.enum_type {
        let name = qualified(&name, enumeration.name.as_deref())?;
        if symbols.insert(name.clone(), owner).is_some() {
            return Err(Error(format!("Project Message Types: Declare Type {name} Only Once")));
        }
    }
    Ok(())
}

fn references(message: &DescriptorProto, pending: &mut Vec<String>) -> Result<(), Error> {
    for field in message.field.iter().chain(&message.extension) {
        for name in [&field.type_name, &field.extendee].into_iter().flatten() {
            if !name.starts_with('.') {
                return Err(Error(format!("Project Message Types: Fully Qualify Type Reference {name}")));
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
