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

pub(crate) async fn perform(inner: &crate::manager::Inner, action: &FileAction) -> Result<FileCompleted> {
    match action {
        FileAction::Create { id, metadata, staging, resumable } => {
            let actual = if *resumable {
                inner.bounded(inner.primary.create_production(metadata)).await?
            } else {
                inner.bounded(inner.primary.create_file(metadata)).await?
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
            inner.bounded(inner.primary.fence_production(staging, *expected, *next)).await?;
            Ok(FileCompleted::Resumed(*next))
        }
        FileAction::Write { staging, offset, bytes, .. } => {
            inner.bounded(inner.primary.write_file(staging, *offset, bytes)).await?;
            Ok(FileCompleted::Written)
        }
        FileAction::Finish { staging, metadata, target, length, sha256, .. } => {
            let position = StorageCursor { history: inner.primary.history().clone(), sequence: target.sequence };
            let reference = inner.bounded(inner.primary.finish_file(staging, &position, *length, *sha256)).await?;
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
                inner.bounded(inner.primary.abandon_file(extra)).await?;
            }
            inner.bounded(inner.primary.abandon_file(staging)).await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Abort { staging, extra, .. } => {
            if let Some(extra) = extra {
                inner.primary.abort_file(extra).await?;
            }
            inner.primary.abort_file(staging).await?;
            Ok(FileCompleted::Aborted)
        }
        FileAction::Refuse { .. } => Err(size_error()),
    }
}

/* === Private Functions === */

/* === Tests === */
