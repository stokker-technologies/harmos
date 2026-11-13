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
use super::{GuestInventory, PluginDefinition, PluginFailure};
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginDefinitions {
    pub(super) inventory: GuestInventory,
    pub(super) rebuilt: BTreeSet<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PluginDefinitions {
    pub fn new(definitions: impl IntoIterator<Item = PluginDefinition>, rebuilt: impl IntoIterator<Item = String>) -> Result<Self> {
        let inventory = GuestInventory::resolve(definitions).map_err(PluginFailure::Plan)?;
        let rebuilt: BTreeSet<_> = rebuilt.into_iter().collect();
        for id in &rebuilt {
            if !inventory.ordered().iter().any(|definition| definition.id == *id) {
                return Err(PluginFailure::Plan(super::Error::Unknown { id: id.clone() }).into());
            }
        }

        Ok(Self { inventory, rebuilt })
    }

    pub fn inventory(&self) -> &GuestInventory {
        &self.inventory
    }

    pub fn definitions(&self) -> impl Iterator<Item = &PluginDefinition> {
        self.inventory.ordered().iter()
    }

    pub fn host_requirements(&self) -> impl Iterator<Item = (&PluginDefinition, &harmos_operations::inventory::contracts::Required)> {
        self.definitions().flat_map(move |definition| {
            let selected = self.inventory.required_providers(&definition.id);
            definition
                .contributions
                .required_contracts
                .iter()
                .flatten()
                .filter(move |required| !selected.contains_key(&required.contract.operation().id))
                .map(move |required| (definition, required))
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
