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

use harmos_protocol::v1::Declarations;

// Standard Library Imports
use std::collections::BTreeSet;

// Third Party Imports

// Local Imports
use super::{Error, v1};
use harmos_protocol::schema;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn verify(declarations: &Declarations) -> Result<(), Error> {
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
        if !matches!(v1::Visibility::try_from(operation.visibility), Ok(v1::Visibility::Protected | v1::Visibility::Public)) {
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

/* === Tests === */
