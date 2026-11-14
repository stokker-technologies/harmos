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
use harmos_runtime::Identifier;
use harmos_runtime::config::PluginSettings;
use harmos_runtime::guest::{Host, Loaded};
use std::sync::Arc;

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
    // Keep Operation Streaming in the Host
    let resources = runtime.operations.context().resource::<schematic_operations::schematic::upload::AnalysisResources>()?;
    let streams = runtime.streams.clone();
    resources.progress.get_or_init(|| Arc::new(move |row| streams.publish(row)));
    let registration = harmos_runtime::operations::Registration::new(harmos_runtime::operations::Lane::default())
        .protobuf::<schematic_operations::AnalyzeSchematic>()?
        .protobuf::<schematic_operations::AnalyzeUpload>()?
        .protobuf::<schematic_operations::AnalyzeExchange>()?
        .protobuf::<schematic_operations::transfer::Upload>()?
        .protobuf::<schematic_operations::transfer::Download>()?
        .protobuf::<schematic_operations::transfer::Put>()?
        .protobuf::<schematic_operations::transfer::Take>()?;
    runtime.register_batch(registration).await?;

    runtime
        .plugin
        .register_guest(
            loaded,
            PluginSettings {
                configuration: b"null".to_vec(),
                host_operations: [Identifier::new("harmos.host.v1.plugins.Settings/List").unwrap()].into_iter().collect(),
                ..PluginSettings::default()
            },
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
