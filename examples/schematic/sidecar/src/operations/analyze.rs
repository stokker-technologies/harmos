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
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::Error;
use harmos_domain::MemorySize;
use schematic_domain::operations::AnalyzeSchematic;
use schematic_domain::operations::schematic::analyze::Analysis;
use schematic_domain::operations::schematic::analyze::Finding;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct Analyzing;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_sidecar::operation]
impl Analyzing {
    async fn initialize(_: Arc<crate::Resources>, _: &harmos_sidecar::Supervisor) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(
        &mut self, _context: &harmos_sidecar::Supervisor, request: &mut AnalyzeSchematic,
        output: &mut harmos_sidecar::operations::StreamOutput<Finding>,
    ) -> Result<Analysis, Error> {
        let findings = [
            format!("{} symbols", request.schematic.symbols.len()),
            format!("{} connections", request.schematic.connections.len()),
            "Analysis complete".to_owned(),
        ];
        for text in findings {
            let finding = Finding { text };
            let bytes = finding.memory_size();
            output.send_with(bytes, || Ok(finding)).await?;
        }
        Ok(Analysis { checked: 3 })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
