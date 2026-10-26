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
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[tracing::instrument(name = "storage.staging.perform.perform", level = "debug", skip_all)]
pub(crate) async fn perform(inner: &crate::manager::Inner, action: &FileAction) -> Result<FileCompleted> {
    match action {
        FileAction::Create { id, metadata, staging, resumable } => {
            let actual = if *resumable {
                inner.bounded("primary.create_production", inner.primary.create_production(metadata)).await?
            } else {
                inner.bounded("primary.create_file", inner.primary.create_file(metadata)).await?
            };
            if actual != *staging {
                if let Some(file) = inner.state.lock().unwrap().files.get_mut(id) {
                    file.extra = Some(actual);
                }
                return Err(Error::InvalidInput { field: "file staging", reason: "backend changed staging identity" });
            }
            Ok(FileCompleted::Created)
        }
        FileAction::Resume { staging, expected, next, .. } => {
            inner.bounded("primary.fence_production", inner.primary.fence_production(staging, *expected, *next)).await?;
            Ok(FileCompleted::Resumed(*next))
        }
        FileAction::Write { staging, offset, bytes, .. } => {
            inner.bounded("primary.write_file", inner.primary.write_file(staging, *offset, bytes.clone())).await?;
            Ok(FileCompleted::Written)
        }
        FileAction::Finish { staging, metadata, target, length, sha256, .. } => {
            let position = StorageCursor { history: inner.primary.history().clone(), sequence: target.sequence };
            let reference = inner.bounded("primary.finish_file", inner.primary.finish_file(staging, &position, *length, *sha256)).await?;
            if reference.position != position
                || reference.metadata != **metadata
                || reference.length != *length
                || reference.sha256 != *sha256
            {
                return Err(Error::InvalidInput { field: "file finish", reason: "backend changed immutable file reference" });
            }
            Ok(FileCompleted::Finished(Box::new(reference)))
        }
        FileAction::Abandon { staging, extra, .. } => {
            if let Some(extra) = extra {
                inner.bounded("primary.abandon_file", inner.primary.abandon_file(extra)).await?;
            }
            inner.bounded("primary.abandon_file", inner.primary.abandon_file(staging)).await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Abort { staging, extra, .. } => {
            if let Some(extra) = extra {
                tracing::Instrument::instrument(
                    inner.primary.abort_file(extra),
                    tracing::debug_span!("storage.backend", method = "primary.abort_file"),
                )
                .await?;
            }
            tracing::Instrument::instrument(
                inner.primary.abort_file(staging),
                tracing::debug_span!("storage.backend", method = "primary.abort_file"),
            )
            .await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Refuse { .. } => Err(size_error()),
    }
}

/* === Private Functions === */

/* === Tests === */
