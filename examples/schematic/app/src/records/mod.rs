/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod export;

/* === Re-Exports === */

pub use export::{ExportCompleted, ExportFormat};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Records {
    ExportCompleted(ExportCompleted),
}
impl From<ExportCompleted> for Records {
    fn from(value: ExportCompleted) -> Self {
        Self::ExportCompleted(value)
    }
}
impl harmos_runtime::state::CatalogCodec for Records {
    fn identify(&self) -> (harmos_runtime::DefinitionId, u32) {
        match self {
            Self::ExportCompleted(_) => <ExportCompleted as harmos_runtime::state::Record>::ENTRY.definition(),
        }
    }
    fn encode<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::ExportCompleted(value) => serde::Serialize::serialize(value, serializer),
        }
    }
}
pub fn records_registry() -> harmos_runtime::state::RecordRegistry<Records> {
    harmos_runtime::state::RecordRegistry::new().entry(
        <ExportCompleted as harmos_runtime::state::Record>::ENTRY,
        Records::ExportCompleted,
        |value| {
            let Records::ExportCompleted(value) = value;
            Some(value)
        },
    )
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
