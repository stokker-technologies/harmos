/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::collections::{BTreeMap, BTreeSet};

use crate::operations::OperationDefinition;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the operation definitions differ from the admitted catalog")]
    Mismatch,

    #[error("operation inventory is invalid: {source}")]
    Invalid {
        #[source]
        source: crate::Error,
    },
}

pub fn validate(operations: &[OperationDefinition]) -> Result<(), Error> {
    let mut ids = BTreeSet::new();
    for operation in operations {
        operation.validate().map_err(|source| Error::Invalid { source })?;
        if !ids.insert(&operation.id) {
            return Err(Error::Invalid {
                source: crate::Error::DuplicateOperation { id: operation.id.as_str().to_owned() },
            });
        }
    }
    Ok(())
}

pub fn validate_match(expected: &[OperationDefinition], actual: &[OperationDefinition]) -> Result<(), Error> {
    validate(expected)?;
    validate(actual)?;
    let expected: BTreeMap<_, _> = expected.iter().map(|entry| (&entry.id, entry)).collect();
    if expected.len() != actual.len() || actual.iter().any(|entry| expected.get(&entry.id).is_none_or(|declared| *declared != entry)) {
        return Err(Error::Mismatch);
    }
    Ok(())
}
