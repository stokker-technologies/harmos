/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private WebAssembly Operations Retained Until the Guest Protocol Migration.

/* === Modules === */

pub mod schematic;
pub mod transfer;

/* === Re-Exports === */

pub use schematic::{AnalyzeSchematic, ProposeRoute, ValidateSchematic};

/* === Enums === */

#[harmos_operations::catalog]
pub enum Operations {
    ValidateSchematic(ValidateSchematic),
    ProposeRoute(ProposeRoute),
    AnalyzeSchematic(AnalyzeSchematic),
}
