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
use serde::{Deserialize, Serialize};
// Local Imports
use super::StreamDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub id: String,
    pub description: String,
    pub activation: Activation,
    pub streams: Vec<String>,
    pub demand_keys: Vec<String>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    #[default]
    Plugin,

    Demand,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("observer `{id}` is invalid: {reason}")]
    Invalid { id: String, reason: &'static str },
}

/* === Traits === */

/* === Public Functions === */

pub fn validate(observers: &[Declaration], grants: &[StreamDeclaration]) -> Result<(), Error> {
    let mut ids = BTreeSet::new();
    for observer in observers {
        let invalid = |reason| Error::Invalid { id: observer.id.clone(), reason };
        if observer.id.trim().is_empty() || observer.id.chars().any(char::is_control) || !ids.insert(&observer.id) {
            return Err(invalid("identities must be nonempty, unique, and contain no control characters"));
        }
        let mut keys = BTreeSet::new();
        for stream in &observer.streams {
            if !keys.insert(stream) || !grants.iter().any(|grant| grant.id == *stream) {
                return Err(invalid("outputs must be unique members of the artifact publication grant"));
            }
        }
        for key in &observer.demand_keys {
            if key.trim().is_empty() || key.chars().any(char::is_control) || !keys.insert(key) {
                return Err(invalid("demand keys must be nonempty, unique, and distinct from outputs"));
            }
        }
        if observer.activation == Activation::Demand && keys.is_empty() {
            return Err(invalid("demand activation requires an output or demand key"));
        }
    }
    Ok(())
}
