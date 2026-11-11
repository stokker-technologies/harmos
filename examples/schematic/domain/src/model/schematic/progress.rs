/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Progress Published by the Native Analysis Sidecar.

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

#[derive(harmos_domain::Row, Default, PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnalysisProgress {
    pub title: String,
    pub checked: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_domain::streams::StreamRow for AnalysisProgress {
    const ENTRY: harmos_domain::StreamEntry<Self> =
        harmos_domain::StreamEntry::new("schematic.analysis_progress", 16).label("Analysis Progress");
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
