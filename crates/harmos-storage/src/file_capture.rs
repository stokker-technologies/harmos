/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Capture A File Range Into Independent Immutable Durable Storage.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::{FileReference, Result, StorageManager};
use harmos_domain::model::storage::files::CaptureConsistency;
use harmos_domain::operations::storage::{CaptureFileRequest, ReadFileRequest};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub async fn capture_file(&self, request: CaptureFileRequest) -> Result<FileReference> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.capture_file", outcome = "cancelled"));
        context
            .result(span, async {
                let mut reader = self.read_file(ReadFileRequest { source: request.source, range: request.range }).await?;
                let mut target = self.create_file(request.metadata).await?;
                while let Some((batch, memory)) = reader.next_raw(false).await? {
                    target.write_all(batch.bytes).await?;
                    drop(memory);
                }
                reader.completion()?;
                if request.consistency == CaptureConsistency::Stable {
                    reader.check_unchanged().await?;
                }
                let reference = target.finish().await?;
                self.flush_until(&reference.position).await?;
                Ok(reference)
            })
            .await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
