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

// Third Party Imports
use serde::Serialize;
// Local Imports
use crate::identity::artifact_subset;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Registration {
    pub namespace: String,
    pub schema: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct Registrar {
    registrations: Vec<Registration>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("`{namespace}` is not a portable registration namespace")]
    InvalidNamespace { namespace: String },

    #[error("registration `{namespace}` has a zero schema ordinal")]
    ZeroSchema { namespace: String },

    #[error("registration namespace `{namespace}` was declared more than once")]
    DuplicateNamespace { namespace: String },

    #[error("registration `{namespace}` could not be encoded: {source}")]
    Encode {
        namespace: String,

        #[source]
        source: serde_json::Error,
    },
}

/* === Traits === */

/* === Implementations === */

impl Registrar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Serialize>(&mut self, namespace: impl Into<String>, schema: u32, value: &T) -> Result<(), RegistrationError> {
        let namespace = namespace.into();
        if !artifact_subset(&namespace) {
            return Err(RegistrationError::InvalidNamespace { namespace });
        }
        if schema == 0 {
            return Err(RegistrationError::ZeroSchema { namespace });
        }
        if self.registrations.iter().any(|entry| entry.namespace == namespace) {
            return Err(RegistrationError::DuplicateNamespace { namespace });
        }
        let payload = serde_json::to_vec(value).map_err(|source| RegistrationError::Encode { namespace: namespace.clone(), source })?;
        self.registrations.push(Registration { namespace, schema, payload });
        Ok(())
    }

    pub fn finish(mut self) -> Vec<Registration> {
        self.registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        self.registrations
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
