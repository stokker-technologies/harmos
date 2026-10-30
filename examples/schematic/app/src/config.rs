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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Settings {
    pub journal_sync_entries: NonZeroUsize,
    recordings: PathBuf,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct LaunchConfig {
    pub document: crate::document::Document,
    pub settings: Settings,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Settings {
    pub fn new(recordings: impl Into<PathBuf>) -> Self {
        Self {
            journal_sync_entries: NonZeroUsize::new(1).unwrap(),
            recordings: recordings.into(),
        }
    }

    pub fn recordings(&self) -> &Path {
        &self.recordings
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
