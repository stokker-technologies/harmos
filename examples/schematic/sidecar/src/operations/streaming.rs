/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bounded Streaming Analysis Retains Its Permit Until EOF Or Cancellation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::{Error, Resources};
use harmos_domain::MemorySize;
use harmos_sidecar::Supervisor;
use harmos_sidecar::operations::{Host, StreamInput, StreamOutput};
use schematic_domain::model::Snapshot;
use schematic_domain::model::schematic::progress::AnalysisProgress;
use schematic_domain::operations::schematic::analyze::{Analysis, Finding};
use schematic_domain::operations::{AnalyzeExchange, AnalyzeUpload};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Uploading {
    resources: Arc<Resources>,
}
pub(crate) struct Exchanging {
    resources: Arc<Resources>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[harmos_sidecar::operation]
impl Uploading {
    fn classify(error: &Error) -> harmos_sidecar::operations::FailureKind {
        error.kind()
    }
    async fn initialize(resources: Arc<Resources>, _: &Supervisor) -> Self {
        Self { resources }
    }

    async fn run(
        &mut self, context: &Supervisor, request: &mut AnalyzeUpload, input: &mut StreamInput<Snapshot>,
    ) -> Result<Analysis, Error> {
        validate_title(&request.title)?;
        let _permit = self.resources.analysis.acquire().await.map_err(|_| Error::Unavailable)?;
        let mut checked = 0u32;
        while let Some(snapshot) = input.recv().await {
            snapshot.validate().map_err(|_| Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(Error::InvalidInput)?;
            context.publish_on::<AnalysisProgress>(&AnalysisProgress { title: request.title.clone(), checked }).await?;
        }
        Ok(Analysis { checked })
    }
}

#[harmos_sidecar::operation]
impl Exchanging {
    fn classify(error: &Error) -> harmos_sidecar::operations::FailureKind {
        error.kind()
    }
    async fn initialize(resources: Arc<Resources>, _: &Supervisor) -> Self {
        Self { resources }
    }

    async fn run(
        &mut self, context: &Supervisor, request: &mut AnalyzeExchange, input: &mut StreamInput<Snapshot>,
        output: &mut StreamOutput<Finding>,
    ) -> Result<Analysis, Error> {
        validate_title(&request.title)?;
        let _permit = self.resources.analysis.acquire().await.map_err(|_| Error::Unavailable)?;
        let mut checked = 0u32;
        while let Some(snapshot) = input.recv().await {
            snapshot.validate().map_err(|_| Error::InvalidInput)?;
            checked = checked.checked_add(1).ok_or(Error::InvalidInput)?;
            let finding = Finding { text: format!("{} Symbols, {} Connections", snapshot.symbols.len(), snapshot.connections.len()) };
            output.send_with(finding.memory_size(), || Ok(finding)).await?;
            context.publish_on::<AnalysisProgress>(&AnalysisProgress { title: request.title.clone(), checked }).await?;
        }
        Ok(Analysis { checked })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn validate_title(title: &str) -> Result<(), Error> {
    if title.trim().is_empty() || title.len() > 128 {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

/* === Tests === */
