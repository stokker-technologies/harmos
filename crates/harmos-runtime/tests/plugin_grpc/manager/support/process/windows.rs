/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain the Exact Windows Process Through Termination

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
// Third Party Imports
use tokio::process::Command;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn command(id: u32) -> Command {
    let script = format!(
        "$ErrorActionPreference='Stop'; $process=Get-Process -Id {id}; $null=$process.Handle; [Console]::Out.WriteLine('ready'); [Console]::Out.Flush(); $control=[Console]::ReadLine(); if ($control -eq 'kill') {{ $process.Kill() }}; $process.WaitForExit(); $process.Dispose()"
    );
    let mut command = Command::new("pwsh");
    command.args(["-NoProfile", "-NonInteractive", "-Command", &script]).creation_flags(0x0800_0000);
    command
}

/* === Tests === */
