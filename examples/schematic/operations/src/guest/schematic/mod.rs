/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Operations Over One Schematic Snapshot, Served by Every Delivery.

/* === Modules === */

pub mod analyze;
pub mod propose;
pub mod validate;

/* === Re-Exports === */

#[cfg(feature = "runtime")]
pub use analyze::analyze_schematic_operation;
pub use analyze::{ANALYZE_SCHEMATIC_DESCRIPTION, ANALYZE_SCHEMATIC_OPERATION, AnalyzeSchematic};
#[cfg(feature = "runtime")]
pub use propose::propose_route_operation;
pub use propose::{PROPOSE_ROUTE_DESCRIPTION, PROPOSE_ROUTE_OPERATION, ProposeRoute};
#[cfg(feature = "runtime")]
pub use validate::validate_schematic_operation;
pub use validate::{VALIDATE_SCHEMATIC_DESCRIPTION, VALIDATE_SCHEMATIC_OPERATION, ValidateSchematic};

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

/* === Private Functions === */

/* === Tests === */
