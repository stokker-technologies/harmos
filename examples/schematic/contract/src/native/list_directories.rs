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
pub struct Request {}

#[harmos_contract::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub directories: Vec<String>,
}

#[harmos_contract::operation(
    id = "schematic.list_directories",
    request = Request,
    response = Response,
    policy = crate::operations::POLICY,
    visibility = harmos_contract::operations::Visibility::Shared,
    version = 1,
    grpc(package = "schematic.v1", service = "Schematic", method = "ListDirectories"),
)]
pub struct ListDirectories;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
