/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub mod guest;
pub mod sidecar;

use std::path::Path;

use crate::app::{Runtime, SchematicEditor};
use harmos::guest::Loaded;

pub enum Plugin {
    Guest { manager: harmos::plugin::PluginHost<SchematicEditor> },
    Sidecar { manager: harmos::plugin::PluginHost<SchematicEditor> },
}

impl Plugin {
    pub async fn guest(runtime: &Runtime, loaded: Loaded<SchematicEditor>) -> harmos::Result<Self> {
        guest::start(runtime, loaded).await?;
        Ok(Self::Guest { manager: runtime.plugin.clone() })
    }

    pub async fn sidecar(runtime: &Runtime, path: &Path, directory: &Path) -> harmos::Result<Self> {
        sidecar::start(runtime, path, directory).await?;
        Ok(Self::Sidecar { manager: runtime.plugin.clone() })
    }

    pub async fn stop(self) -> harmos::Result<()> {
        match self {
            Self::Guest { manager } => manager.stop(guest::NAME).await,
            Self::Sidecar { manager } => manager.stop(sidecar::NAME).await,
        }
    }
}
