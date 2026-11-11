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
use harmos::guest::Loaded;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum Plugin {
    Guest { manager: harmos::plugin::PluginHost<SchematicEditor> },
    Sidecar { attached: harmos::plugin::grpc::Attached },
}

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub async fn guest(runtime: &Runtime, loaded: Loaded<SchematicEditor>) -> harmos::Result<Self> {
        guest::start(runtime, loaded).await?;
        Ok(Self::Guest { manager: runtime.plugin.clone() })
    }

    pub async fn sidecar(runtime: &Runtime, path: &Path, directory: &Path) -> harmos::Result<Self> {
        let attached = sidecar::start(runtime, path, directory).await?;
        Ok(Self::Sidecar { attached })
    }

    pub async fn stop(self) -> harmos::Result<()> {
        match self {
            Self::Guest { manager } => manager.stop(guest::NAME).await,
            Self::Sidecar { attached } => attached
                .stop("Example Complete")
                .await
                .map_err(|source| harmos::Error::OperationFailed { operation: sidecar::NAME.into(), source }),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
