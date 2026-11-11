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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin() -> harmos_sdk_grpc::Plugin {
    let directory = std::env::var_os("SCHEMATIC_DIRECTORY").map(std::path::PathBuf::from).unwrap_or_else(|| ".".into());
    schematic_operations::native::plugin(directory)
}

/* === Private Functions === */

/* === Tests === */
