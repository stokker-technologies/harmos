/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Observe an Owned Process Until Exit

/* === Modules === */

#[cfg(unix)]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::process::{ExitStatus, Stdio};
// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Child,
};
// Local Imports
#[cfg(unix)]
use unix::command;
#[cfg(target_os = "windows")]
use windows::command;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Process {
    child: Child,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Process {
    pub async fn observe(id: u32) -> Self {
        let mut child =
            command(id).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true).spawn().unwrap();
        let mut ready = BufReader::new(child.stdout.take().unwrap()).lines();
        ready.next_line().await.unwrap().expect("the observer retains the exact process before acknowledging readiness");
        Self { child }
    }
    pub async fn kill(mut self) -> ExitStatus {
        self.child.stdin.as_mut().unwrap().write_all(b"kill\n").await.unwrap();
        self.exited().await
    }
    pub async fn exited(mut self) -> ExitStatus {
        drop(self.child.stdin.take());
        self.child.wait().await.unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
