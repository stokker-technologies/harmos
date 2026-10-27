/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Streaming Analysis Over A Sequence Of Schematic Snapshots.

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
pub struct AnalyzeUploadRequest {
    pub title: String,
}

#[harmos::operation(
    id = "schematic_analyze_upload", request = AnalyzeUploadRequest, infer_request, response = super::analyze::Analysis, input = schematic_domain::model::Snapshot,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeUpload"),
)]
pub struct AnalyzeUpload {
    #[cfg(feature = "runtime")]
    resources: std::sync::Arc<crate::resources::AnalysisResources>,
}

#[harmos::message]
pub struct AnalyzeExchangeRequest {
    pub title: String,
}

#[harmos::operation(
    id = "schematic_analyze_exchange", request = AnalyzeExchangeRequest, infer_request, response = super::analyze::Analysis, input = schematic_domain::model::Snapshot,
    output = super::analyze::Finding,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeExchange"),
)]
pub struct AnalyzeExchange {
    #[cfg(feature = "runtime")]
    resources: std::sync::Arc<crate::resources::AnalysisResources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for AnalyzeUpload {
    type Request = AnalyzeUploadRequest;
    type Response = super::analyze::Analysis;
    type Error = crate::Error;
    type Endpoints = harmos::operations::StreamInput<schematic_domain::model::Snapshot>;

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { resources: context.dependency()? })
    }

    fn classify(error: &Self::Error) -> harmos::operations::FailureKind {
        match error {
            crate::Error::InvalidInput => harmos::operations::FailureKind::Invalid,
            _ => harmos::operations::FailureKind::Permanent,
        }
    }

    async fn run(
        &mut self, context: harmos::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        if request.title.trim().is_empty() || request.title.len() > 128 {
            return Err(crate::Error::InvalidInput);
        }
        let _permit = self.resources.analysis.acquire().await.map_err(|_| crate::Error::Unavailable)?;
        let mut checked = 0u32;
        while let Some(snapshot) = endpoints.recv().await {
            snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;
            let progress = schematic_domain::model::schematic::progress::AnalysisProgress { title: request.title.clone(), checked };
            (self.resources.publisher)(context.clone(), progress).await?;
        }
        Ok(super::analyze::Analysis { checked }.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for AnalyzeExchange {
    type Request = AnalyzeExchangeRequest;
    type Response = super::analyze::Analysis;
    type Error = crate::Error;
    type Endpoints =
        (harmos::operations::StreamInput<schematic_domain::model::Snapshot>, harmos::operations::StreamOutput<super::analyze::Finding>);

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { resources: context.dependency()? })
    }

    fn classify(error: &Self::Error) -> harmos::operations::FailureKind {
        match error {
            crate::Error::InvalidInput => harmos::operations::FailureKind::Invalid,
            _ => harmos::operations::FailureKind::Permanent,
        }
    }

    async fn run(
        &mut self, context: harmos::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        if request.title.trim().is_empty() || request.title.len() > 128 {
            return Err(crate::Error::InvalidInput);
        }
        let _permit = self.resources.analysis.acquire().await.map_err(|_| crate::Error::Unavailable)?;
        let mut checked = 0u32;
        while let Some(snapshot) = endpoints.0.recv().await {
            snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;
            let finding =
                super::analyze::Finding { text: format!("{} Symbols, {} Connections", snapshot.symbols.len(), snapshot.connections.len()) };
            endpoints.1.send_with(finding.memory_size(), || Ok(finding)).await?;
            let progress = schematic_domain::model::schematic::progress::AnalysisProgress { title: request.title.clone(), checked };
            (self.resources.publisher)(context.clone(), progress).await?;
        }
        Ok(super::analyze::Analysis { checked }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
