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

// Local Imports
use crate::Result;
use harmos_runtime::operations::StreamSettings;
use schematic_app::app::Runtime;
use schematic_app::domain::model::Snapshot;
use schematic_app::operations::guest::AnalyzeSchematic;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn run(runtime: &Runtime, schematic: Snapshot, native: bool) -> Result<()> {
    // Drain a Bounded Streaming Analysis
    let settings = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    if native {
        let request = schematic_app::operations::protocol::AnalyzeSchematic {
            schematic: Some(schematic.clone().into()),
        };
        let mut analysis = runtime.operations.download_protobuf::<schematic_app::operations::AnalyzeSchematic>(request, settings)?;
        while let Some(finding) = analysis.next().await? {
            println!("Analysis: {}", finding.text);
        }
        let response = analysis.join().await?;
        println!("Analysis settled: {} findings", response.checked);
        let request = schematic_app::operations::protocol::AnalyzeSchematic { schematic: Some(schematic.into()) };
        runtime.operations.download_protobuf::<schematic_app::operations::AnalyzeSchematic>(request, settings)?.cancel_and_settle().await?;
        println!("Cancelled analysis and waited for settlement");
    } else {
        let request = AnalyzeSchematic { schematic: schematic.clone() };
        let mut analysis = runtime.operations.download::<AnalyzeSchematic>(request, settings)?;
        while let Some(finding) = analysis.next().await? {
            println!("Analysis: {}", finding.text);
        }
        let response = analysis.join().await?;
        println!("Analysis settled: {} findings", response.checked);

        // Cancel and Settle Analysis
        let request = AnalyzeSchematic { schematic };
        let analysis = runtime.operations.download::<AnalyzeSchematic>(request, settings)?;
        analysis.cancel_and_settle().await?;
        println!("Cancelled analysis and waited for settlement");
    }

    // Only the Native Sidecar Has Filesystem Access
    if native {
        let request = schematic_app::protocol::ListDirectories {};
        let response = schematic_app::services::sidecar::call(runtime, request).await?;
        println!("Native directories: {:?}", response.directories);
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
