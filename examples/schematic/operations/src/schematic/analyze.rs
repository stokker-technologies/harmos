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

#[cfg(feature = "runtime")]
use harmos::MemorySize;

// Standard Library Imports

// Third Party Imports

// Local Imports

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
    id = "schematic_analyze",
    response = Analysis,
    output = Finding,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
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

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let request = &mut self;

        let findings = [
            format!("{} symbols", request.schematic.symbols.len()),
            format!("{} connections", request.schematic.connections.len()),
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
