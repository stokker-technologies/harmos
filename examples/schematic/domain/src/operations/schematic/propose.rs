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
    id = "schematic.route",
    response = Route,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
    version = 1,
    grpc(package = "schematic.v1", service = "Schematic", method = "ProposeRoute"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ProposeRoute {
    pub from: crate::model::Point,
    pub to: crate::model::Point,
}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<crate::model::Point>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
