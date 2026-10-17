/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Progress Published By The Native Analysis Sidecar.

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
#[harmos_domain::stream(id = "schematic.analysis_progress", window = 16)]
#[harmos_domain::message]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnalysisProgress {
    pub title: String,
    pub checked: u32,
}


/* === Enums === */
#[harmos_domain::catalog]
pub enum Streams {
    Progress(AnalysisProgress),
}


/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
