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
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

// Third Party Imports

// Local Imports
use crate::error::ConfigError;

/* === Constants === */
pub const RECORDINGS: &str = "SCHEMATIC_RECORDINGS";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Settings {
    pub journal_sync_entries: NonZeroUsize,
    recordings: PathBuf,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Settings {
    pub fn new(recordings: impl Into<PathBuf>) -> Self {
        Self { journal_sync_entries: NonZeroUsize::new(1).unwrap(), recordings: recordings.into() }
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        match std::env::var(RECORDINGS) {
            Ok(recordings) if !recordings.is_empty() => Ok(Self::new(recordings)),
            Ok(_) => Err(ConfigError::Empty { variable: RECORDINGS }),
            Err(std::env::VarError::NotPresent) => Err(ConfigError::Missing { variable: RECORDINGS }),
            Err(std::env::VarError::NotUnicode(_)) => Err(ConfigError::NotUnicode { variable: RECORDINGS }),
        }
    }

    pub fn recordings(&self) -> &Path {
        &self.recordings
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
