/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

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

pub struct Plugin {
    manager: harmos::plugin::PluginHost<SchematicEditor>,
}

/* === Traits === */

/* === Implementations === */

impl Plugin {
    pub async fn sidecar(runtime: &Runtime, path: &Path, directory: &Path) -> harmos::Result<Self> {
        sidecar::start(runtime, path, directory).await?;
        Ok(Self { manager: runtime.plugin.clone() })
    }

    pub async fn stop(self) -> harmos::Result<()> {
        self.manager.stop(sidecar::NAME).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
