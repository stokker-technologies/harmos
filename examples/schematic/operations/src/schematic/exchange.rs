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

pub struct AnalyzeExchange;

/* === Implementations === */

impl ProtobufOperation for AnalyzeExchange {
    type Request = crate::protocol::SchematicAnalyzeExchangeHeader;
    type Response = crate::protocol::AnalysisResult;
    type Endpoints = (
        harmos_operations::execution::operations::ProtobufInput<crate::protocol::Snapshot>,
        harmos_operations::execution::operations::ProtobufOutput<crate::protocol::Finding>,
    );
    const ID: &'static str = "schematic.host.v1.Schematic/AnalyzeExchange";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety =
        harmos_operations::execution::operations::ReplaySafety::Idempotent;
    const GRPC: Option<harmos_operations::execution::operations::Grpc> =
        Some(harmos_operations::execution::operations::Grpc::new("schematic.host.v1", "Schematic", "AnalyzeExchange"));
    const RESOURCES: &'static [harmos_operations::Resource] = &[harmos_operations::Resource::of::<super::upload::AnalysisResources>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            if request.title.trim().is_empty() || request.title.len() > 128 {
                return Err(crate::Error::InvalidInput);
            }
            let resources = ctx.resource::<super::upload::AnalysisResources>()?;
            let _permit = resources.analysis.acquire().await.map_err(|_| crate::Error::Unavailable)?;
            let mut checked = 0u32;
            while let Some(item) = endpoints.0.recv().await {
                let (snapshot, _claims) = item.into_owned_request()?;
                let snapshot: schematic_domain::model::Snapshot = snapshot.into();
                snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
                checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;
                endpoints
                    .1
                    .send(crate::protocol::Finding {
                        text: format!("{} Symbols, {} Connections", snapshot.symbols.len(), snapshot.connections.len()),
                    })
                    .await?;
                resources
                    .publish(schematic_domain::model::schematic::progress::AnalysisProgress { title: request.title.clone(), checked })?;
            }
            Ok(crate::protocol::AnalysisResult { checked })
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
