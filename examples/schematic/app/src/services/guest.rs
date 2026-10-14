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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::app::{Runtime, SchematicEditor};
use crate::lanes::Lanes;
use harmos::config::PluginSettings;
use harmos::guest::{Host, Loaded};
use schematic_contract::operations::Operations;

/* === Constants === */
pub const NAME: &str = "schematic-tools";

const SDK_ENCODING_MEMORY_BYTES: NonZeroUsize = NonZeroUsize::new(256).unwrap();

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn start(runtime: &Runtime, loaded: Loaded<SchematicEditor>) -> harmos::Result<()> {
    if loaded.declaration().id != NAME {
        return Err(harmos::Error::InvalidInput {
            field: "schematic tools artifact",
            reason: "the inspected component has a different owner",
        });
    }
    let settings = PluginSettings {
        encoding_memory_bytes: SDK_ENCODING_MEMORY_BYTES,
        ..Default::default()
    };
    runtime
        .plugin
        .register_guest::<Operations>(loaded, settings, Lanes::Open)
        .await?;
    runtime.plugin.initialize().await
}

pub fn load(
    runtime: &Runtime,
    component: &[u8],
) -> Result<Loaded<SchematicEditor>, harmos::guest::Error> {
    let host = Host::new(runtime)?;
    host.load(component)
}

/* === Private Functions === */
pub(super) fn failure(source: impl std::error::Error + Send + Sync + 'static) -> harmos::Error {
    harmos::Error::OperationFailed {
        operation: NAME.into(),
        source: Arc::new(source),
    }
}

/* === Tests === */
