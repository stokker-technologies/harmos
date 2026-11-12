/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hidden Managed Process Startup

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;
// Third Party Imports
use tokio::process::Command;
// Local Imports

/* === Constants === */

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn command(executable: &Path) -> Command {
    let mut command = Command::new(executable);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/* === Tests === */
