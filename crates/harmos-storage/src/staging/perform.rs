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
pub(crate) async fn perform(inner: &Arc<crate::manager::Inner>, action: &FileAction) -> Result<FileCompleted> {
    match action {
        FileAction::Create { id, metadata, staging, resumable } => {
            let actual = if *resumable {
                inner.bounded("primary.create_file_production", inner.primary.create_file_production(metadata)).await?
            } else {
                inner.bounded("primary.create_file_staging", inner.primary.create_file_staging(metadata)).await?
            };
            if actual != *staging {
                if let Some(file) = inner.state.lock().unwrap().files.get_mut(id) {
                    file.extra = Some(actual);
                }
                return Err(Error::InvalidInput {
                    field: "file staging",
                    reason: "backend changed staging identity",
                });
            }
            Ok(FileCompleted::Created)
        }
        FileAction::Resume { staging, expected, next, .. } => {
            inner
                .bounded("primary.set_file_writer_generation", inner.primary.set_file_writer_generation(staging, *expected, *next))
                .await?;
            Ok(FileCompleted::Resumed(*next))
        }
        FileAction::Write { staging, offset, bytes, .. } => {
            inner.bounded("primary.append_file_bytes", inner.primary.append_file_bytes(staging, *offset, bytes.clone())).await?;
            Ok(FileCompleted::Written)
        }
        FileAction::Replay { staging, offset, bytes, .. } => {
            let mut checked = 0;
            while checked < bytes.len() {
                let maximum = std::num::NonZeroUsize::new(bytes.len() - checked).expect("remaining accepted prefix");
                let retained = inner
                    .bounded(
                        "primary.get_file_production_bytes",
                        inner.primary.get_file_production_bytes(staging, *offset + checked as u64, maximum),
                    )
                    .await?;
                if retained.is_empty() || retained.len() > maximum.get() || retained.capacity() > maximum.get() {
                    return Err(Error::InvalidInput {
                        field: "AppendFileBytes",
                        reason: "backend returned an empty or oversized accepted prefix",
                    });
                }
                if retained.as_slice() != &bytes.as_slice()[checked..checked + retained.len()] {
                    return Ok(FileCompleted::Replayed(false));
                }
                checked += retained.len();
            }
            Ok(FileCompleted::Replayed(true))
        }
        FileAction::Finish {
            staging,
            metadata,
            target,
            length,
            sha256,
            ..
        } => {
            let position = StorageCursor {
                history: inner.primary.get_history().clone(),
                sequence: target.sequence,
            };
            let reference = inner
                .bounded("primary.publish_file_production", inner.primary.publish_file_production(staging, &position, *length, *sha256))
                .await?;
            if reference.position != position
                || reference.metadata != **metadata
                || reference.length != *length
                || reference.sha256 != *sha256
            {
                return Err(Error::InvalidInput {
                    field: "file finish",
                    reason: "backend changed immutable file reference",
                });
            }
            Ok(FileCompleted::Finished(Box::new(reference)))
        }
        FileAction::Abandon { staging, extra, .. } => {
            if let Some(extra) = extra {
                inner.bounded("primary.delete_file_production", inner.primary.delete_file_production(extra)).await?;
            }
            inner.bounded("primary.delete_file_production", inner.primary.delete_file_production(staging)).await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Abort { staging, extra, .. } => {
            if let Some(extra) = extra {
                tracing::Instrument::instrument(
                    inner.primary.delete_file_staging(extra),
                    tracing::debug_span!("storage.backend", method = "primary.delete_file_staging"),
                )
                .await?;
            }
            tracing::Instrument::instrument(
                inner.primary.delete_file_staging(staging),
                tracing::debug_span!("storage.backend", method = "primary.delete_file_staging"),
            )
            .await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Refuse { .. } => Err(size_error()),
    }
}

/* === Private Functions === */

/* === Tests === */
