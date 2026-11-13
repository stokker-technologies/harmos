/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resolve Native Test Binaries in Cargo and Nextest.

/* === Modules === */

mod catalog_fault;

/* === Re-Exports === */

pub use catalog_fault::{CatalogFault, Fault};

/* === Imports === */

// Standard Library Imports
use std::error::Error;
use std::{
    ffi::OsString,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct PluginProcess {
    pub address: String,
    child: Child,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PluginProcess {
    pub fn start(executable: &OsString) -> Result<Self, Box<dyn Error>> {
        let child = Command::new(executable)
            .arg("--remote")
            .env("HARMOS_ADDRESS", "127.0.0.1:0")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut process = Self { address: String::new(), child };
        let stdout = process.child.stdout.take().ok_or("Missing Standard Output")?;
        let mut lines = BufReader::new(stdout).lines();
        let line = lines.next().ok_or("Missing Readiness Line")??;
        process.address = line.strip_prefix("harmos listening ").ok_or("Invalid Readiness Line")?.into();
        Ok(process)
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/* === Public Functions === */

pub fn binary(name: &str, fallback: &str) -> OsString {
    std::env::var_os(format!("NEXTEST_BIN_EXE_{name}")).unwrap_or_else(|| fallback.into())
}

/* === Private Functions === */

/* === Tests === */
