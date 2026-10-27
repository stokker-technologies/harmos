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

#[harmos::message]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzeSchematicRequest {
    pub schematic: schematic_domain::model::Snapshot,
}

#[harmos::operation(
    id = "schematic_analyze",
    request = AnalyzeSchematicRequest, infer_request, response = Analysis,
    output = Finding,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "Analyze"),
)]
pub struct AnalyzeSchematic {}

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
    type Request = AnalyzeSchematicRequest;
    type Response = Analysis;
    type Error = crate::Error;
    type Endpoints = harmos::operations::StreamOutput<Finding>;

    async fn initialize(_: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self {})
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let findings = [
            format!("{} symbols", request.schematic.symbols.len()),
            format!("{} connections", request.schematic.connections.len()),
            "Analysis complete".to_owned(),
        ];
        for text in findings {
            let finding = Finding { text };
            let bytes = finding.memory_size();
            endpoints.send_with(bytes, || Ok(finding)).await?;
        }
        Ok(Analysis { checked: 3 }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
