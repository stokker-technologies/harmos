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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Bindings, Component, Lane, OperationDefinition, OperationSpec, RequestShape};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Register = dyn Fn(&mut Bindings<'_>, &'static str, Arc<Component>) -> Result<()> + Send + Sync;

/* === Structs === */

#[derive(Clone, Default)]
pub struct ComponentCatalog {
    entries: Vec<ComponentEntry>,
}

#[derive(Clone)]
struct ComponentEntry {
    definition: OperationDefinition,
    register: Arc<Register>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ComponentCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operation<Q: RequestShape>(mut self, specification: OperationSpec<Q>) -> Result<Self> {
        let definition = specification.definition().map_err(|source| Error::OperationDeclaration { source })?;
        if self.entries.iter().any(|entry| entry.definition.id == definition.id) {
            return Err(Error::AlreadyRegistered {
                domain: "component operation",
                name: definition.id.to_string(),
            });
        }
        self.entries.push(ComponentEntry {
            definition,
            register: Arc::new(move |bindings, lane, component| super::component::register::<Q>(bindings, specification, lane, component)),
        });
        Ok(self)
    }

    pub fn definitions(&self) -> Vec<OperationDefinition> {
        self.entries.iter().map(|entry| entry.definition.clone()).collect()
    }

    pub(super) fn register(&self, bindings: &mut Bindings<'_>, lane: Lane, component: Arc<Component>) -> Result<()> {
        let previous = bindings.entries.len();
        for entry in &self.entries {
            if let Err(error) = (entry.register)(bindings, lane.name(), Arc::clone(&component)) {
                bindings.entries.truncate(previous);
                return Err(error);
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
