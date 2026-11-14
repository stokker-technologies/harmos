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

// Third Party Imports
use serde_json::{Value, json};

// Local Imports
use crate::Result;
use harmos_runtime::operations::StreamSettings;
use schematic_app::app::Runtime;
use schematic_app::domain::model::Snapshot;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn run(runtime: &Runtime, schematic: Snapshot, native: bool) -> Result<Value> {
    // Drain a Bounded Streaming Analysis
    let settings = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let request = schematic_app::operations::protocol::SchematicAnalyzeRequest {
        schematic: Some(schematic.clone().into()),
    };
    let mut analysis = runtime.operations.download_protobuf::<schematic_app::operations::AnalyzeSchematic>(request, settings)?;
    let mut findings = 0;
    while let Some(finding) = analysis.next().await? {
        findings += 1;
        println!("Analysis: {}", finding.text);
    }
    let response = analysis.join().await?;
    println!("Analysis settled: {} findings", response.checked);
    let request = schematic_app::operations::protocol::SchematicAnalyzeRequest { schematic: Some(schematic.into()) };
    runtime.operations.download_protobuf::<schematic_app::operations::AnalyzeSchematic>(request, settings)?.cancel_and_settle().await?;
    println!("Cancelled analysis and waited for settlement");

    // Only the Native Sidecar Has Filesystem Access
    let directories = if native {
        let request = schematic_app::protocol::plugin::SchematicListDirectoriesRequest {};
        let response = schematic_app::services::call(runtime, request).await?;
        println!("Native directories: {:?}", response.directories);
        Some(response.directories)
    } else {
        None
    };
    Ok(json!({"findings": findings, "checked": response.checked, "cancelled": true, "directories": directories}))
}

/* === Private Functions === */

/* === Tests === */
