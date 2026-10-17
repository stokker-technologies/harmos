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

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos_domain::message]
#[harmos_domain::operation(
    id = "schematic.validate",
    response = Validation,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
    version = 1,
    grpc(package = "schematic.v1", service = "Schematic", method = "Validate"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateSchematic {
    pub schematic: crate::model::Snapshot,
}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Validation {
    pub diagnostics: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
