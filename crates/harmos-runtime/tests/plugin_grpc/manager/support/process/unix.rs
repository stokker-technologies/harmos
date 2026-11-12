/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Observe the Exact Linux Process Through Its Retained Descriptor

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::PathBuf;
// Third Party Imports
use tokio::process::Command;
// Local Imports

/* === Constants === */

const OBSERVE: &str = r#"
import os, select, signal, sys
fd = os.pidfd_open(int(sys.argv[1]))
print('ready', flush=True)
if sys.stdin.readline().strip() == 'kill':
    signal.pidfd_send_signal(fd, signal.SIGKILL)
select.select([fd], [], [])
os.close(fd)
"#;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn command(id: u32) -> Command {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let mut command = Command::new(manifest.join("../../.venv/bin/python"));
    command.args(["-c", OBSERVE, &id.to_string()]);
    command
}

/* === Tests === */
