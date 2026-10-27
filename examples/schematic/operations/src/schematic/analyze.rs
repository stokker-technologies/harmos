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
#[cfg(feature = "runtime")]
use harmos::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub text: String,
}

#[harmos::operation(
    id = "schematic.analyze",
    response = Analysis,
    output = Finding,
    policy = crate::POLICY,
    visibility = Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "Analyze"),
)]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzeSchematic {
    pub schematic: schematic_domain::model::Snapshot,
}

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub checked: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for AnalyzeSchematic {
    type Error = crate::Error;

    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<harmos::Response<Analysis>, Self::Error> {
        let findings = [
            format!("{} symbols", self.schematic.symbols.len()),
            format!("{} connections", self.schematic.connections.len()),
            "Analysis complete".to_owned(),
        ];
        for text in findings {
            let finding = Finding { text };
            let bytes = finding.memory_size();
            ctx.streams().send_with(bytes, || Ok(finding)).await?;
        }
        Ok(Analysis { checked: 3 }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
