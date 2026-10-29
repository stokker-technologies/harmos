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
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct AnalysisResources {
    pub(crate) analysis: tokio::sync::Semaphore,
    progress_sequence: AtomicU64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Default for AnalysisResources {
    fn default() -> Self {
        Self {
            analysis: tokio::sync::Semaphore::new(1),
            progress_sequence: AtomicU64::new(0),
        }
    }
}

impl AnalysisResources {
    // The Analysis Permit Keeps Publications In Sequence Order Across Calls.
    pub(crate) fn next_progress_sequence(&self) -> Result<u64, crate::Error> {
        let advance = |sequence: u64| sequence.checked_add(1);
        let previous = self.progress_sequence.try_update(Ordering::Relaxed, Ordering::Relaxed, advance);
        let previous = previous.map_err(|_| crate::Error::Unavailable)?;
        Ok(previous + 1)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
