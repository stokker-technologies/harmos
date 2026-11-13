/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use harmos_operations::execution::operations::{ExecutionContext, ProtobufOperation, ServingError, Visibility};

/* === Constants === */

const POLICY: harmos_operations::execution::operations::OperationPolicy = harmos_operations::execution::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_operations::execution::operations::RetryPolicy::Never,
};

/* === Structs === */

pub struct AnalyzeSchematic;

/* === Implementations === */

impl ProtobufOperation for AnalyzeSchematic {
    type Request = crate::protocol::AnalyzeSchematic;
    type Response = crate::protocol::Analysis;
    type Endpoints = harmos_operations::execution::operations::ProtobufOutput<crate::protocol::Finding>;
    const ID: &'static str = "schematic.analyze";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety =
        harmos_operations::execution::operations::ReplaySafety::Idempotent;
    const GRPC: Option<harmos_operations::execution::operations::Grpc> =
        Some(harmos_operations::execution::operations::Grpc::new("schematic.v1", "Schematic", "Analyze"));
    async fn run(_ctx: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            let schematic = request.schematic.ok_or(crate::Error::InvalidInput)?;
            let findings = [
                format!("{} symbols", schematic.symbols.len()),
                format!("{} connections", schematic.connections.len()),
                "Analysis complete".to_owned(),
            ];
            for text in findings {
                endpoints.send(crate::protocol::Finding { text }).await?;
            }
            Ok(crate::protocol::Analysis { checked: 3 })
        }
        .await;
        result.map_err(|error| ServingError::Native(std::sync::Arc::new(error)))
    }
    fn classify(error: &(dyn std::error::Error + Send + Sync + 'static)) -> harmos_operations::execution::operations::FailureKind {
        match error.downcast_ref::<crate::Error>() {
            Some(crate::Error::InvalidInput) => harmos_operations::execution::operations::FailureKind::Invalid,
            _ => harmos_operations::execution::operations::FailureKind::Permanent,
        }
    }
}
