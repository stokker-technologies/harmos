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
// Third Party Imports

// Local Imports
use super::PluginDefinition;
use crate::Result;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginGeneration(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginStatus {
    pub id: String,
    pub definition: Option<PluginDefinition>,
    pub active_definition: Option<PluginDefinition>,
    pub generation: Option<PluginGeneration>,
    pub state: PluginState,
    // Startup Attempts Begun Since This Plugin Identity Was Registered.
    pub attempts: u64,
    // The Most Recent Startup Or Cleanup Failure, Retained Across Recovery.
    pub last_failure: Option<String>,
    pub cleanup_pending: bool,
    // Classification Of The Last Failure, Retained Across Recovery.
    pub failure_class: Option<PluginFailureClass>,
    // A Restart Deadline; Cleanup Must Settle Before Another Attempt Begins.
    pub next_retry: Option<tokio::time::Instant>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginFailureClass {
    Recoverable,
    Terminal,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginState {
    Registered,
    Initializing,
    Ready,
    Stopping,
    Failed,
}

/* === Traits === */

/* === Implementations === */

impl PluginGeneration {
    pub fn get(self) -> u64 {
        self.0
    }

    pub(super) fn fresh() -> Result<Self> {
        crate::plugin::fresh_generation().map(Self)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
