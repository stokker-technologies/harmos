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
use std::fmt;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExportCompleted {
    pub format: ExportFormat,
    pub symbols: u32,
}

/* === Enums === */

#[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ExportFormat {
    Svg,
    Netlist,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::state::Record for ExportCompleted {
    const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("export.completed");
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Svg => "SVG",
            Self::Netlist => "netlist",
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
