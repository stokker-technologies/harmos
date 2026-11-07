/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// AnalyzeExchange Contracts and Execution.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
#[cfg(feature = "runtime")]
use super::upload::AnalysisResources;
#[cfg(feature = "runtime")]
use harmos::MemorySize;

/* === Constants === */

const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
    timeout: std::time::Duration::from_secs(10),
    attempt_timeout: std::time::Duration::from_secs(10),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos::operations::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(resources = [AnalysisResources], operations = [harmos::builtins::streams::Publish],
    id = "schematic.analyze.exchange", response = super::analyze::Analysis, input = schematic_domain::model::Snapshot,
    output = super::analyze::Finding,
    policy = POLICY,
    visibility = Public,
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
impl harmos::Operation for AnalyzeExchange {
    type Error = crate::Error;

    fn classify(error: &Self::Error) -> harmos::operations::FailureKind {
        match error {
            crate::Error::InvalidInput => harmos::operations::FailureKind::Invalid,
            _ => harmos::operations::FailureKind::Permanent,
        }
    }

    async fn run(self, ctx: harmos::Context<'_, Self>) -> Result<harmos::Response<super::analyze::Analysis>, Self::Error> {
        let resources = ctx.resource::<AnalysisResources>();

        // Check / Title Presence
        let title_present = !self.title.trim().is_empty();

        // Check / Title Size
        let title_fits = self.title.len() <= 128;
        if !(title_present && title_fits) {
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
            let bytes = finding.memory_size();
            let output = &ctx.streams().1;
            let make = || Ok(finding);
            output.send_with(bytes, make).await?;
            let progress = schematic_domain::model::schematic::progress::AnalysisProgress { title: self.title.clone(), checked };
            let sequence = resources.next_progress_sequence()?;
            let payload = harmos::Message::encode(&progress);
            // Publish Analysis Progress
            let request = harmos::builtins::streams::Publish {
                stream: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::ID.to_owned(),
                sequence: Some(sequence),
                published_unix_micros: None,
                context: None,
                payload,
                following: Vec::new(),
            };
            ctx.operation(request).await?;
        }
        Ok(super::analyze::Analysis { checked }.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
