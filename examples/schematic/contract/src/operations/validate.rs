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
#[harmos_contract::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub schematic: crate::model::Snapshot,
}

#[harmos_contract::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub diagnostics: Vec<String>,
}

#[harmos_contract::operation(
    id = "schematic.validate",
    request = Request,
    response = Response,

    policy = super::POLICY,
    visibility = harmos_contract::operations::Visibility::Shared,
    replay = harmos_contract::operations::ReplaySafety::Idempotent,
    version = 1,
    grpc(package = "schematic.v1", service = "Schematic", method = "Validate"),
)]
pub struct ValidateSchematic;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
