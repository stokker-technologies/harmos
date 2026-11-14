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

pub struct AnalyzeUpload;

/* === Implementations === */

impl ProtobufOperation for AnalyzeUpload {
    type Request = crate::protocol::SchematicAnalyzeUploadHeader;
    type Response = crate::protocol::SchematicAnalyzeUploadResponse;
    type Endpoints = harmos_operations::execution::operations::ProtobufInput<crate::protocol::Snapshot>;
    const ID: &'static str = "schematic.analyze.upload";
    const POLICY: harmos_operations::execution::operations::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: harmos_operations::execution::operations::ReplaySafety =
        harmos_operations::execution::operations::ReplaySafety::Idempotent;
    const GRPC: Option<harmos_operations::execution::operations::Grpc> =
        Some(harmos_operations::execution::operations::Grpc::new("schematic.host.v1", "Schematic", "AnalyzeUpload"));
    const RESOURCES: &'static [harmos_operations::Resource] = &[harmos_operations::Resource::of::<super::upload::AnalysisResources>()];
    async fn run(ctx: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let result: Result<Self::Response, crate::Error> = async {
            if request.title.trim().is_empty() || request.title.len() > 128 {
                return Err(crate::Error::InvalidInput);
            }
            let resources = ctx.resource::<super::upload::AnalysisResources>()?;
            let _permit = resources.analysis.acquire().await.map_err(|_| crate::Error::Unavailable)?;
            let mut checked = 0u32;
            while let Some(item) = endpoints.recv().await {
                let (snapshot, _claims) = item.into_owned_request()?;
                let snapshot: schematic_domain::model::Snapshot = snapshot.into();
                snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
                checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;

                resources
                    .publish(schematic_domain::model::schematic::progress::AnalysisProgress { title: request.title.clone(), checked })?;
            }
            Ok(crate::protocol::SchematicAnalyzeUploadResponse { checked })
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

/* === Structs === */

pub struct AnalysisResources {
    pub(crate) analysis: tokio::sync::Semaphore,
    pub progress: std::sync::OnceLock<std::sync::Arc<dyn Fn(schematic_domain::model::schematic::progress::AnalysisProgress) + Send + Sync>>,
}

/* === Implementations === */

impl Default for AnalysisResources {
    fn default() -> Self {
        Self {
            analysis: tokio::sync::Semaphore::new(1),
            progress: Default::default(),
        }
    }
}
impl AnalysisResources {
    pub(crate) fn publish(&self, row: schematic_domain::model::schematic::progress::AnalysisProgress) -> Result<(), crate::Error> {
        self.progress.get().ok_or(crate::Error::Unavailable)?(row);
        Ok(())
    }
}
