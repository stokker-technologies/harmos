/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use tokio::sync::broadcast;

use super::{PluginGeneration, PluginState};

pub struct PluginEvents {
    pub(super) receiver: broadcast::Receiver<PluginEvent>,
}

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

impl PluginEvents {
    pub async fn next(&mut self) -> Option<PluginEvent> {
        match self.receiver.recv().await {
            Ok(event) => Some(event),
            Err(broadcast::error::RecvError::Lagged(missed)) => Some(PluginEvent::Lagged { missed }),
            Err(broadcast::error::RecvError::Closed) => None,
        }
    }
}
