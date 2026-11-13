/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod voltage;

/* === Re-Exports === */

pub use voltage::NetVoltage;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn stream_definitions() -> Vec<harmos_runtime::streams::Definition> {
    vec![
        <NetVoltage as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap(),
        <schematic_domain::model::schematic::progress::AnalysisProgress as harmos_runtime::streams::StreamRow>::ENTRY.build().unwrap(),
    ]
}

/* === Private Functions === */

/* === Tests === */
