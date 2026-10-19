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
    id = "schematic.list_directories",
    response = Directories,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    version = 1,
    grpc(package = "schematic.v1", service = "Schematic", method = "ListDirectories"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct ListDirectories {}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Directories {
    pub directories: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
