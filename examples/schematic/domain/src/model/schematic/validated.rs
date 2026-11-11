/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Stored Schematic That Enforces Its Domain Invariants.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug)]
pub struct ValidatedSchematic {
    pub id: String,
    pub schematic: super::Snapshot,
}

/* === Enums === */

pub fn objects_catalog() -> harmos_domain::model::storage::ObjectCatalog {
    harmos_domain::model::storage::ObjectCatalog::new().entry(harmos_domain::model::storage::ObjectDefinition::of::<ValidatedSchematic>())
}

/* === Traits === */

/* === Implementations === */

impl harmos_domain::Identified for ValidatedSchematic {
    const ID: &'static str = "schematic.validated";
}

impl harmos_domain::model::storage::catalog::StoredObject for ValidatedSchematic {
    type Key = str;
    const RETENTION: harmos_domain::model::storage::ObjectRetention = harmos_domain::model::storage::ObjectRetention::Latest;
    fn key(&self) -> &str {
        &self.id
    }
    fn validate(&self) -> harmos_domain::Result<()> {
        // Check / Snapshot Identity
        let id_present = !self.id.trim().is_empty();

        // Check / Schematic
        let schematic_valid = self.schematic.validate().is_ok();
        if !(id_present && schematic_valid) {
            return Err(harmos_domain::Error::InvalidInput {
                field: "schematic",
                reason: "a stored schematic needs an identity and valid connections",
            });
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
