/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod operations;

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

pub fn plugin() -> harmos::Plugin {
    let directory = std::env::var_os("SCHEMATIC_DIRECTORY").map(std::path::PathBuf::from).unwrap_or_else(|| ".".into());
    operations::plugin(directory)
}

/* === Private Functions === */

/* === Tests === */
