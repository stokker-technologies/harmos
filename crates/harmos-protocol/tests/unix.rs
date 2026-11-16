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
use std::{error::Error, fs, os::unix::fs::PermissionsExt, path::Path};

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

pub(crate) fn prepare_compiler(compiler: &Path) -> Result<(), Box<dyn Error>> {
    let mut permissions = fs::metadata(compiler)?.permissions();
    permissions.set_mode(permissions.mode() | 0o100);
    fs::set_permissions(compiler, permissions)?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
