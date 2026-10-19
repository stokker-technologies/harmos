/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Streaming Analysis Over A Sequence Of Schematic Snapshots.

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
    id = "schematic.analyze_upload", version = 1,
    response = super::analyze::Analysis, input = crate::model::Snapshot,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeUpload"),
)]
pub struct AnalyzeUpload {
    pub title: String,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "schematic.analyze_exchange", version = 1,
    response = super::analyze::Analysis, input = crate::model::Snapshot,
    output = super::analyze::Finding,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeExchange"),
)]
pub struct AnalyzeExchange {
    pub title: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
