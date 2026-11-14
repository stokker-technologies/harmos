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

// Local Imports
use crate::operations::{Contract, OperationDefinition};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the operation definitions differ from the admitted catalog")]
    Mismatch,

    #[error("operation inventory is invalid: {source}")]
    Invalid {
        #[source]
        source: crate::operations::Error,
    },
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn validate(operations: &[OperationDefinition], contracts: &[Contract]) -> Result<(), Error> {
    let mut ids = BTreeSet::new();
    for operation in operations {
        if contracts.iter().any(|contract| contract.operation() == operation && contract.protobuf().is_some()) {
            harmos_protocol::schema::plugin_operation_identity(operation.id.as_str()).map_err(|_| Error::Invalid {
                source: crate::operations::Error::InvalidOperation { id: operation.id.to_string() },
            })?;
            operation.policy.validate().map_err(|source| Error::Invalid { source: source.into() })?;
        } else {
            operation.validate().map_err(|source| Error::Invalid { source })?;
        }
        if !ids.insert(&operation.id) {
            return Err(Error::Invalid {
                source: crate::operations::Error::DuplicateOperation { id: operation.id.as_str().to_owned() },
            });
        }
    }
    Ok(())
}

pub fn validate_match(expected: &[OperationDefinition], actual: &[OperationDefinition]) -> Result<(), Error> {
    validate(expected, &[])?;
    validate(actual, &[])?;
    let expected: BTreeMap<_, _> = expected.iter().map(|entry| (&entry.id, entry)).collect();
    if expected.len() != actual.len() || actual.iter().any(|entry| expected.get(&entry.id).is_none_or(|declared| *declared != entry)) {
        return Err(Error::Mismatch);
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
