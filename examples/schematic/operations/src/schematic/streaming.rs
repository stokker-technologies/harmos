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

#[harmos::operation(resources = [crate::resources::AnalysisResources], operations = [harmos::builtins::streams::Publish],
    id = "schematic_analyze_upload", response = super::analyze::Analysis, input = schematic_domain::model::Snapshot,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeUpload"),
)]
pub struct AnalyzeUpload {
    pub title: String,
}

#[harmos::operation(resources = [crate::resources::AnalysisResources], operations = [harmos::builtins::streams::Publish],
    id = "schematic_analyze_exchange", response = super::analyze::Analysis, input = schematic_domain::model::Snapshot,
    output = super::analyze::Finding,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeExchange"),
)]
pub struct AnalyzeExchange {
    pub title: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for AnalyzeUpload {
    type Error = crate::Error;

    fn classify(error: &Self::Error) -> harmos::operations::FailureKind {
        match error {
            crate::Error::InvalidInput => harmos::operations::FailureKind::Invalid,
            _ => harmos::operations::FailureKind::Permanent,
        }
    }

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let resources = ctx.resource::<crate::resources::AnalysisResources>();
        let request = &mut self;

        if request.title.trim().is_empty() || request.title.len() > 128 {
            return Err(crate::Error::InvalidInput);
        }
        let _permit = resources.analysis.acquire().await.map_err(|_| crate::Error::Unavailable)?;
        let mut checked = 0u32;
        while let Some(snapshot) = ctx.streams().recv().await {
            snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;
            let progress = schematic_domain::model::schematic::progress::AnalysisProgress { title: request.title.clone(), checked };
            ctx.operation(harmos::builtins::streams::Publish {
                stream: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::ID.to_owned(),
                schema: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::VERSION,
                sequence: None,
                published_unix_micros: None,
                context: None,
                payload: harmos::Message::encode(&progress),
                following: Vec::new(),
            })
            .await?;
        }
        Ok(super::analyze::Analysis { checked }.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for AnalyzeExchange {
    type Error = crate::Error;

    fn classify(error: &Self::Error) -> harmos::operations::FailureKind {
        match error {
            crate::Error::InvalidInput => harmos::operations::FailureKind::Invalid,
            _ => harmos::operations::FailureKind::Permanent,
        }
    }

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let resources = ctx.resource::<crate::resources::AnalysisResources>();
        let request = &mut self;

        if request.title.trim().is_empty() || request.title.len() > 128 {
            return Err(crate::Error::InvalidInput);
        }
        let _permit = resources.analysis.acquire().await.map_err(|_| crate::Error::Unavailable)?;
        let mut checked = 0u32;
        while let Some(snapshot) = ctx.streams().0.recv().await {
            snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;
            let finding = super::analyze::Finding {
                text: format!("{} Symbols, {} Connections", snapshot.symbols.len(), snapshot.connections.len()),
            };
            ctx.streams().1.send_with(finding.memory_size(), || Ok(finding)).await?;
            let progress = schematic_domain::model::schematic::progress::AnalysisProgress { title: request.title.clone(), checked };
            ctx.operation(harmos::builtins::streams::Publish {
                stream: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::ID.to_owned(),
                schema: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::VERSION,
                sequence: None,
                published_unix_micros: None,
                context: None,
                payload: harmos::Message::encode(&progress),
                following: Vec::new(),
            })
            .await?;
        }
        Ok(super::analyze::Analysis { checked }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
