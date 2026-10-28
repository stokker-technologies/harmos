/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::num::NonZeroUsize;

use crate::Result;
use harmos::operations::StreamSettings;
use schematic_app::app::Runtime;
use schematic_app::domain::model::Snapshot;
use schematic_app::operations::{AnalyzeSchematic, ListDirectories};

pub async fn run(runtime: &Runtime, schematic: Snapshot, native: bool) -> Result<()> {
    // Drain A Bounded Streaming Analysis
    let settings = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let request = AnalyzeSchematic { schematic };
    let mut analysis = runtime.operations.download::<AnalyzeSchematic>(request.clone(), settings)?;
    while let Some(finding) = analysis.next().await? {
        println!("Analysis: {}", finding.text);
    }
    let response = analysis.join().await?;
    println!("Analysis settled: {} findings", response.checked);

    // Cancellation Includes Physical Settlement
    let analysis = runtime.operations.download::<AnalyzeSchematic>(request, settings)?;
    analysis.cancel_and_settle().await?;
    println!("Cancelled analysis and waited for settlement");

    // Only The Native Sidecar Has Filesystem Access
    if native {
        let request = ListDirectories {};
        let response = runtime.operation(request).run().await?;
        println!("Native directories: {:?}", response.directories);
    }
    Ok(())
}
