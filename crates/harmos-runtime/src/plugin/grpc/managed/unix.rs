/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Process Commands

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;

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

pub(super) fn command(executable: &Path) -> Command {
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
    let mut command = Command::new(executable);
    #[cfg(target_os = "linux")]
    die_with_parent(&mut command);
    command
}

/// Kill the Child When the Host Process Dies, Including the Race Where It Died Before `prctl`.
#[cfg(target_os = "linux")]
fn die_with_parent(command: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: The Hook Calls Only Async-Signal-Safe Functions and Allocates Nothing.
    #[allow(unsafe_code)]
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

/* === Tests === */
