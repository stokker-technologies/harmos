/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Build And Development Process Settings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::Duration;

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct BuildSettings {
    pub backend: BuildBackend,
    pub manifest: std::path::PathBuf,
    pub packages: Vec<String>,
    pub target: Option<String>,
    pub target_directory: Option<std::path::PathBuf>,
    pub release: bool,
    pub timings: bool,
    pub features: Vec<String>,
    pub default_features: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct DevSettings {
    pub debounce: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildBackend {
    Sidecar,
    Guest,
}



/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Default for BuildSettings {
    fn default() -> Self {
        Self {
            backend: BuildBackend::Sidecar,
            manifest: "Cargo.toml".into(),
            packages: Vec::new(),
            target: None,
            target_directory: None,
            release: false,
            timings: false,
            features: Vec::new(),
            default_features: true,
        }
    }
}

impl Default for DevSettings {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(100),
        }
    }
}



/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
