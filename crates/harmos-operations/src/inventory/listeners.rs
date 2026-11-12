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
// Local Imports
use crate::listeners::Definition;
use crate::operations::Contract;

/* === Constants === */

const LIMIT: usize = 4096;

/* === Macros === */

/* === Type Aliases === */

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Schema(#[from] harmos_domain::schema::graph::Error),

    #[error(transparent)]
    Listener(#[from] crate::listeners::Error),

    #[error("multiple listener inventories for {facility} artifact `{owner}`")]
    Duplicate { facility: String, owner: String },

    #[error("listener inventory identity or count is invalid")]
    Inventory,

    #[error("duplicate listener or callback `{id}`")]
    Entry { id: String },

    #[error("listener callback `{id}` is absent or differs from its provided contract")]
    Callback { id: String },
}

/* === Traits === */

/* === Implementations === */

pub fn validate(listeners: &[Definition], provides: &[Contract]) -> Result<(), Error> {
    if listeners.len() > LIMIT {
        return Err(Error::Inventory);
    }
    let mut ids = BTreeSet::new();
    let mut callbacks = BTreeSet::new();
    for listener in listeners {
        let id = listener.callback().operation().id.as_str();
        if !ids.insert(listener.id().as_str()) || !callbacks.insert(id) {
            return Err(Error::Entry { id: id.into() });
        }
        if !provides.iter().any(|contract| contract == listener.callback()) {
            return Err(Error::Callback { id: id.into() });
        }
    }
    Ok(())
}
