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
use tokio::sync::broadcast;
// Local Imports
use super::{PluginGeneration, PluginState};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct PluginEvents {
    pub(super) receiver: broadcast::Receiver<PluginEvent>,
}

/* === Enums === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginEventKind {
    DefinitionChanged,
    Initializing,
    Available,
    Stopping,
    Failed,
    Stopped,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginEvent {
    Changed { id: String, generation: Option<PluginGeneration>, state: PluginState, revision: u64, kind: PluginEventKind },
    Lagged { missed: u64 },
}

/* === Traits === */

/* === Implementations === */
impl PluginEvents {
    pub async fn next(&mut self) -> Option<PluginEvent> {
        match self.receiver.recv().await {
            Ok(event) => Some(event),
            Err(broadcast::error::RecvError::Lagged(missed)) => Some(PluginEvent::Lagged { missed }),
            Err(broadcast::error::RecvError::Closed) => None,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
