/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::path::{Path, PathBuf};
// Third Party Imports

// Local Imports
use crate::error::ConfigError;

/* === Constants === */
pub const BRIDGE: &str = "MINIBENCH_PROBE_BRIDGE";

pub const DEVICE: &str = "MINIBENCH_DEVICE";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Config {
    bridge: PathBuf,
    device: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Config {
    pub fn new(bridge: impl Into<PathBuf>, device: impl Into<String>) -> Self {
        Self {
            bridge: bridge.into(),
            device: device.into(),
        }
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self::new(read(BRIDGE)?, read(DEVICE)?))
    }

    pub fn bridge(&self) -> &Path {
        &self.bridge
    }

    pub fn device(&self) -> &str {
        &self.device
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn read(variable: &'static str) -> Result<String, ConfigError> {
    match std::env::var(variable) {
        Ok(value) if !value.is_empty() => Ok(value),
        Ok(_) => Err(ConfigError::Empty { variable }),
        Err(std::env::VarError::NotPresent) => Err(ConfigError::Missing { variable }),
        Err(std::env::VarError::NotUnicode(_)) => Err(ConfigError::NotUnicode { variable }),
    }
}

/* === Tests === */
