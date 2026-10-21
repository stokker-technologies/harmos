/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod guest;
pub mod sidecar;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;

// Third Party Imports

// Local Imports
use crate::app::{Runtime, SchematicEditor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum Plugin {
    Guest { manager: harmos::plugin::PluginHost<SchematicEditor> },
    Sidecar { manager: harmos::plugin::PluginHost<SchematicEditor> },
}

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub async fn guest(runtime: &Runtime, path: &Path) -> harmos::Result<Self> {
        let bytes = std::fs::read(path).map_err(guest::failure)?;
        let loaded = guest::load(runtime, &bytes).map_err(guest::failure)?;
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
