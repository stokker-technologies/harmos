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
use harmos::services::ServiceLease;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
pub enum Plugin {
    Guest {
        manager: guest::Manager,
        lifecycle: Box<guest::Lifecycle>,
    },
    Sidecar(ServiceLease<SchematicEditor>),
}

/* === Traits === */

/* === Implementations === */
impl Plugin {
    pub async fn guest(runtime: &Runtime, path: &Path) -> harmos::Result<Self> {
        let bytes = std::fs::read(path).map_err(guest::failure)?;
        let loaded = guest::load(runtime, &bytes).map_err(guest::failure)?;
        let (mut manager, mut lifecycle) = guest::lifecycle(runtime, loaded).await?;
        if let Err(error) = manager.initialize(&mut lifecycle).await {
            manager.shutdown(&mut lifecycle).await?;
            return Err(error);
        }
        Ok(Self::Guest {
            manager,
            lifecycle: Box::new(lifecycle),
        })
    }

    pub async fn sidecar(runtime: &Runtime, path: &Path, directory: &Path) -> harmos::Result<Self> {
        let lease = sidecar::start(runtime, path, directory).await?;
        Ok(Self::Sidecar(lease))
    }

    pub async fn stop(self) -> harmos::Result<()> {
        match self {
            Self::Guest {
                mut manager,
                mut lifecycle,
            } => manager.shutdown(lifecycle.as_mut()).await,
            Self::Sidecar(lease) => lease.release().await,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
