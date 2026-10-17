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
use harmos::operations::StreamSettings;
use schematic_app::app::Runtime;
use schematic_app::domain::{
    model::Snapshot,
    operations::{
        AnalyzeSchematic, ListDirectories, directories::list_directories, schematic::analyze,
    },
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn run(runtime: &Runtime, schematic: Snapshot, native: bool) -> Result<()> {
    // Drain A Bounded Streaming Analysis
    let settings = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let request = analyze::Request { schematic };
    let mut analysis = runtime
        .operations
        .download::<AnalyzeSchematic>(request.clone(), settings)?;
    while let Some(finding) = analysis.next().await? {
        println!("Analysis: {}", finding.text);
    }
    let response = analysis.join().await?;
    println!("Analysis settled: {} findings", response.checked);

    // Cancellation Includes Physical Settlement
    let analysis = runtime
        .operations
        .download::<AnalyzeSchematic>(request, settings)?;
    analysis.cancel_and_settle().await?;
    println!("Cancelled analysis and waited for settlement");

    // Only The Native Sidecar Has Filesystem Access
    if native {
        let request = list_directories::Request {};
        let response = runtime
            .operations
            .call_as::<ListDirectories>(request)
            .await?;
        println!("Native directories: {:?}", response.directories);
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
