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
use crate::app::{Runtime, SchematicEditor};
use harmos_runtime::config::PluginSettings;
use harmos_runtime::guest::{Host, Loaded};

/* === Constants === */

pub const NAME: &str = "schematic_tools";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn start(runtime: &Runtime, loaded: Loaded<SchematicEditor>) -> harmos_runtime::Result<()> {
    if loaded.declaration().id != NAME {
        return Err(harmos_runtime::Error::InvalidInput {
            field: "schematic tools artifact",
            reason: "the inspected component has a different owner",
        });
    }
    runtime
        .plugin
        .register_guest(
            harmos_runtime::operations::ComponentCatalog::new()
                .operation(schematic_operations::guest::VALIDATE_SCHEMATIC_OPERATION)?
                .operation(schematic_operations::guest::PROPOSE_ROUTE_OPERATION)?
                .operation(schematic_operations::guest::ANALYZE_SCHEMATIC_OPERATION)?,
            loaded,
            PluginSettings::default(),
            harmos_runtime::operations::Lane::default(),
        )
        .await?;
    runtime.plugin.initialize().await
}

pub fn load(runtime: &Runtime, component: &[u8]) -> Result<Loaded<SchematicEditor>, harmos_runtime::guest::Error> {
    let host = Host::new(runtime)?;
    host.load(component)
}

/* === Private Functions === */

/* === Tests === */
