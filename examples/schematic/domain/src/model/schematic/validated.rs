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
#[harmos_domain::message]
pub struct ValidatedSchematic {
    pub id: String,
    pub schematic: super::Snapshot,
}

/* === Enums === */
#[harmos_domain::catalog]
pub enum Objects {
    Schematic(ValidatedSchematic),
}

/* === Traits === */

/* === Implementations === */
impl harmos_domain::Identified for ValidatedSchematic {
    fn definition() -> (harmos_domain::DefinitionId, u32) {
        (harmos_domain::DefinitionId::new("schematic.validated"), 1)
    }
}

impl harmos_domain::model::storage::catalog::StoredObject for ValidatedSchematic {
    type Key = str;
    const RETENTION: harmos_domain::model::storage::ObjectRetention =
        harmos_domain::model::storage::ObjectRetention::Latest;
    fn key(&self) -> &str {
        &self.id
    }
    fn validate(&self) -> harmos_domain::Result<()> {
        if self.id.trim().is_empty() || self.schematic.validate().is_err() {
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
