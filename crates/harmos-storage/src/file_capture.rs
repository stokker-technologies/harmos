/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Capture A File Range Into Independent Immutable Durable Storage.

use crate::FileRead;
use crate::{FileReference, Result, StorageManager};
use harmos_domain::model::storage::files::CaptureConsistency;

#[derive(Clone, Debug)]
pub struct FileCapture {
    pub source: crate::FileSource,
    pub range: crate::ByteRange,
    pub metadata: crate::FileMetadata,
    pub consistency: CaptureConsistency,
}

impl<C> StorageManager<C> {
    pub async fn capture_file(&self, request: FileCapture) -> Result<FileReference> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.capture_file", outcome = "cancelled"));
        context
            .result(span, async {
                let mut reader = self
                    .read_file(FileRead {
                        source: request.source,
                        range: request.range,
                    })
                    .await?;
                let mut target = self.create_file(request.metadata).await?;
                while let Some(batch) = reader.next_raw(false).await? {
                    let chunk = crate::FileChunk::new(batch.bytes);
                    let offset = target.length();
                    target.write_retained_at(offset, chunk).await?;
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
