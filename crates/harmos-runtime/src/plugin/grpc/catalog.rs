/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// gRPC Declarations Verification

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::model::Declarations;

// Standard Library Imports
use std::collections::BTreeSet;

// Third Party Imports

// Local Imports
use super::{Error, model};
use harmos_protocol::schema;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn verify(plugin: &str, declarations: &Declarations) -> Result<(), Error> {
    verify_identifiers(plugin, declarations)?;
    schema::plugin_collections(declarations).map_err(|error| Error::Invalid(error.to_string()))?;
    if declarations.digest != schema::declarations_digest(declarations) {
        return Err(Error::Invalid("Declarations Digest Mismatch".into()));
    }
    let types = declarations.types.clone().unwrap_or_default();
    prost_reflect::DescriptorPool::from_file_descriptor_set(types.clone())
        .map_err(|error| Error::Invalid(format!("Invalid Declarations Definitions: {error}")))?;
    let mut operations = BTreeSet::new();
    for operation in &declarations.operations {
        schema::plugin_operation_identity(&operation.id).map_err(|error| Error::Invalid(error.to_string()))?;
        if operation.id.is_empty() || !operations.insert(&operation.id) {
            return Err(Error::Invalid(format!("Duplicate or Empty Operation: {}", operation.id)));
        }
        if !matches!(model::Visibility::try_from(operation.visibility), Ok(model::Visibility::Protected | model::Visibility::Public)) {
            return Err(Error::Invalid(format!("Invalid Operation Visibility: {}", operation.id)));
        }
        let fingerprint = schema::fingerprint(&operation.id, &types, &operation.request, &operation.response)
            .map_err(|error| Error::Invalid(error.to_string()))?;
        if operation.fingerprint != fingerprint {
            return Err(Error::Invalid(format!("Operation Fingerprint Mismatch: {}", operation.id)));
        }
    }
    let mut streams = BTreeSet::new();
    for stream in &declarations.streams {
        if stream.id.is_empty() || !streams.insert(&stream.id) {
            return Err(Error::Invalid(format!("Duplicate or Empty Stream: {}", stream.id)));
        }
        schema::message_types(&types, &stream.row).map_err(|error| Error::Invalid(error.to_string()))?;
    }
    let mut sections = BTreeSet::new();
    for section in &declarations.sections {
        if !sections.insert(&section.r#type) {
            return Err(Error::Invalid(format!("Duplicate Section: {}", section.r#type)));
        }
        schema::message_types(&types, &section.r#type).map_err(|error| Error::Invalid(error.to_string()))?;
    }
    crate::plugin::manager::settings::declarations_from(declarations).map_err(|error| Error::Invalid(error.to_string()))?;

    let mut requires = BTreeSet::new();
    for requirement in &declarations.requires {
        schema::operation_identity(&requirement.id).map_err(|error| Error::Invalid(error.to_string()))?;
        if requirement.id.is_empty() || !requires.insert(&requirement.id) || requirement.fingerprint.len() != 32 {
            return Err(Error::Invalid(format!("Invalid Requirement: {}", requirement.id)));
        }
    }
    Ok(())
}

/* === Private Functions === */

fn verify_identifiers(plugin: &str, declarations: &Declarations) -> Result<(), Error> {
    let identifiers = declarations
        .operations
        .iter()
        .map(|operation| operation.id.as_str())
        .chain(declarations.streams.iter().flat_map(|stream| [stream.id.as_str(), stream.row.as_str()]));
    for identifier in identifiers {
        let qualified = identifier.strip_prefix(plugin).and_then(|local| local.strip_prefix('.')).is_some_and(|local| !local.is_empty());
        if !qualified {
            return Err(Error::Invalid(format!(
                "Register Identifier {identifier:?} for Plugin {plugin:?}: rename it to {plugin}.<local> with a non-empty local part and update its references"
            )));
        }
    }
    Ok(())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_qualified_identifiers_are_required() {
        let mut declarations = model::Declarations {
            operations: vec![model::OperationContract {
                id: "conformance.echo".into(),
                ..Default::default()
            }],
            streams: vec![model::StreamDeclaration {
                id: "conformance.rows".into(),
                row: "conformance.Row".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        declarations.requires.push(model::Requirement {
            id: "peer.echo".into(),
            ..Default::default()
        });
        assert!(verify_identifiers("conformance", &declarations).is_ok());
        for identifier in ["", "conformance", "conformance.", "other.echo", "conformance_peer.echo", "echo"] {
            let mut invalid = declarations.clone();
            invalid.operations[0].id = identifier.into();
            assert!(matches!(verify_identifiers("conformance", &invalid), Err(Error::Invalid(_))));

            let mut invalid = declarations.clone();
            invalid.streams[0].id = identifier.into();
            assert!(matches!(verify_identifiers("conformance", &invalid), Err(Error::Invalid(_))));

            let mut invalid = declarations.clone();
            invalid.streams[0].row = identifier.into();
            assert!(matches!(verify_identifiers("conformance", &invalid), Err(Error::Invalid(_))));
        }
    }
}
