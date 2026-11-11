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

#[harmos::streams]
pub enum Streams {
    NetVoltage(NetVoltage),
    AnalysisProgress(schematic_domain::model::schematic::progress::AnalysisProgress),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
