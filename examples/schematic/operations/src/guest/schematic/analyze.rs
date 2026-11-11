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
use harmos_operations::MemorySize;

/* === Constants === */

const POLICY: harmos_operations::operations::OperationPolicy = harmos_operations::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_operations::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub text: String,
}

#[harmos_operations::message(harmos_path = harmos_operations)]
#[derive(harmos_operations::Request)]
#[harmos_path(harmos_operations)]
#[request(response = Analysis, output = Finding)]
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzeSchematic {
    pub schematic: schematic_domain::model::Snapshot,
}

pub static ANALYZE_SCHEMATIC_DESCRIPTION: harmos_operations::operations::WireDeclaration =
    harmos_operations::operations::WireDeclaration::request::<AnalyzeSchematic>("schematic.analyze", POLICY)
        .visibility(harmos_operations::operations::Visibility::Public)
        .replay(harmos_operations::operations::ReplaySafety::Idempotent)
        .grpc(harmos_operations::operations::Grpc::new("schematic.v1", "Schematic", "Analyze"));

pub const ANALYZE_SCHEMATIC_OPERATION: harmos_operations::operations::OperationSpec<AnalyzeSchematic> =
    harmos_operations::operations::OperationSpec::<AnalyzeSchematic>::new(&ANALYZE_SCHEMATIC_DESCRIPTION);

#[harmos_operations::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub checked: u32,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
mod execution {
    use super::*;
    pub async fn analyze_schematic_handler(
        request: AnalyzeSchematic, ctx: harmos_operations::execution::operations::FunctionContext<AnalyzeSchematic>,
    ) -> Result<harmos_operations::Response<Analysis>, crate::Error> {
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

    pub fn analyze_schematic_operation() -> ::core::result::Result<
        harmos_operations::execution::operations::OperationFactory,
        harmos_operations::execution::operations::ServingError,
    > {
        harmos_operations::execution::operations::OperationFactory::function(
            ANALYZE_SCHEMATIC_OPERATION,
            const { &[] },
            analyze_schematic_handler,
            |_| harmos_operations::operations::FailureKind::Permanent,
        )
    }
}

#[cfg(feature = "runtime")]
pub use execution::analyze_schematic_operation;

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
