/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// AnalyzeUpload Contracts and Execution.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
#[cfg(feature = "runtime")]
use std::sync::atomic::{AtomicU64, Ordering};

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
    id = "schematic.analyze.upload", response = super::analyze::Analysis, input = schematic_domain::model::Snapshot,
    policy = POLICY,
    visibility = Public,
    replay = harmos::operations::ReplaySafety::Idempotent,
    grpc(package = "schematic.v1", service = "Schematic", method = "AnalyzeUpload"),
)]
pub struct AnalyzeUpload {
    pub title: String,
}

#[cfg(feature = "runtime")]
pub struct AnalysisResources {
    pub(crate) analysis: tokio::sync::Semaphore,
    progress_sequence: AtomicU64,
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
        while let Some(snapshot) = ctx.streams().recv().await {
            snapshot.validate().map_err(|_| crate::Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(crate::Error::InvalidInput)?;
            let progress = schematic_domain::model::schematic::progress::AnalysisProgress { title: self.title.clone(), checked };
            let sequence = resources.next_progress_sequence()?;
            let payload = harmos::Message::encode(&progress);
            // Publish Analysis Progress
            let request = harmos::builtins::streams::Publish {
                stream: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::ID.to_owned(),
                schema: <schematic_domain::model::schematic::progress::AnalysisProgress as harmos::StreamSpec>::VERSION,
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

#[cfg(feature = "runtime")]
impl Default for AnalysisResources {
    fn default() -> Self {
        Self {
            analysis: tokio::sync::Semaphore::new(1),
            progress_sequence: AtomicU64::new(0),
        }
    }
}

#[cfg(feature = "runtime")]
impl AnalysisResources {
    // The Analysis Permit Keeps Publications in Sequence Order Across Calls.
    pub(crate) fn next_progress_sequence(&self) -> Result<u64, crate::Error> {
        let advance = |sequence: u64| sequence.checked_add(1);
        let previous = self.progress_sequence.try_update(Ordering::Relaxed, Ordering::Relaxed, advance);
        let previous = previous.map_err(|_| crate::Error::Unavailable)?;
        Ok(previous + 1)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
