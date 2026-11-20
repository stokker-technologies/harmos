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

mod declarations_fault;

/* === Re-Exports === */

pub use declarations_fault::{DeclarationsFault, Fault};

/* === Imports === */

// Standard Library Imports
use std::error::Error;
use std::{ffi::OsString, process::Stdio};

// Third Party Imports
use tokio::process::{Child, Command};

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
    /// Start the Plugin in Its Remote Lifecycle and Receive Its Ephemeral Address Through a Launch Announcement.
    pub async fn start(executable: &OsString) -> Result<Self, Box<dyn Error>> {
        let launches = harmos_runtime::plugin::grpc::Launches::new();
        let mut launch = launches.reserve("remote reference plugin").await?;
        let child = Command::new(executable)
            .arg("--remote")
            .envs(launch.environment())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let address = launch.announced().await?.to_string();
        launches.shutdown().await;
        Ok(Self { address, child })
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/* === Public Functions === */

pub fn binary(name: &str, fallback: &str) -> OsString {
    std::env::var_os(format!("NEXTEST_BIN_EXE_{name}")).unwrap_or_else(|| fallback.into())
}

/* === Private Functions === */

/* === Tests === */
