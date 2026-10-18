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
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub text: String,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "schematic.analyze",
    response = Analysis,
    output = Finding,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
    version = 1,
    grpc(package = "schematic.v1", service = "Schematic", method = "Analyze"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzeSchematic {
    pub schematic: crate::model::Snapshot,
}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub checked: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
