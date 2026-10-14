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
use harmos_sidecar::MemorySize;
use schematic_contract::operations::analyze::Finding;
use schematic_contract::operations::analyze::{Request, Response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub(crate) struct AnalyzeSchematic;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
#[harmos_sidecar::operation(declaration = schematic_contract::operations::AnalyzeSchematic, output = Finding)]
impl AnalyzeSchematic {
    async fn initialize(_: Arc<()>, _: &harmos_sidecar::Supervisor) -> Self {
        Self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn run(
        &mut self,
        _context: &harmos_sidecar::Supervisor,
        request: &mut Request,
        output: &mut harmos_sidecar::operations::StreamOutput<Finding>,
    ) -> Result<Response, Error> {
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
        Ok(Response { checked: 3 })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
