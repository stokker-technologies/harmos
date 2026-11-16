/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Prepare the Transferred Protocol Compiler.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{error::Error, path::Path};

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

pub(crate) fn prepare_compiler(_compiler: &Path) -> Result<(), Box<dyn Error>> {
    // Transferred Windows Executables Need No Permission-Bit Repair.
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
