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
use std::{env, path::PathBuf};

// Third Party Imports

// Local Imports
use harmos::plugin::Plugin;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn plugin() -> Plugin {
    let directory = env::var_os("SCHEMATIC_DIRECTORY").map(PathBuf::from).unwrap_or_else(|| ".".into());
    operations::plugin(directory)
}

/* === Private Functions === */

/* === Tests === */
