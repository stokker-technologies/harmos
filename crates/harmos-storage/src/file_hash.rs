/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash A Bounded File Range Without Retaining The Complete Payload.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use crate::FileRead;
use crate::{Result, StorageManager};
use harmos_domain::model::storage::files::FileHash;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    pub async fn hash_file(&self, request: FileRead) -> Result<FileHash> {
        let context = crate::execution::Context::current();
        let span = context.sync(|| tracing::debug_span!("storage.file.hash_file", outcome = "cancelled"));
        context
            .result(span, async {
                let mut reader = self
                    .read_file(FileRead {
                        source: request.source,
                        range: request.range,
                    })
                    .await?;
                let mut hash = Sha256::new();
                let local = reader.is_local();
                while let Some(batch) = reader.next().await? {
                    if local {
                        hash.update(batch.bytes());
                    }
                }
                let completion = reader.completion()?;
                Ok(reader.stored_hash()?.unwrap_or_else(|| FileHash {
                    bytes_read: completion.bytes,
                    sha256: hash.finalize().into(),
                }))
            })
            .await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

impl crate::Snapshot {
    /// Hash An Exact Range Of A Validated Immutable Reference At This Durable Image.
    pub async fn hash_file(&self, file: &crate::FileReference, range: crate::ByteRange) -> Result<FileHash> {
        self.check_file_reference(file, "GetFileHash").await?;
        let end = range.end(file.length)?;
        let mut offset = range.offset;
        let mut hash = Sha256::new();
        while offset < end {
            let maximum =
                std::num::NonZeroUsize::new((end - offset).min(self.inner.settings.response.batch_bytes.get() as u64) as usize).unwrap();
            let bytes = self.read_file(file, offset, maximum).await?;
            if bytes.is_empty() || bytes.len() > maximum.get() || bytes.capacity() > maximum.get() {
                return Err(crate::Error::backend(
                    "GetFileHash",
                    crate::BackendFailure::Corrupt,
                    std::io::Error::other("provider returned an empty or oversized content range"),
                ));
            }
            offset += bytes.len() as u64;
            hash.update(bytes);
        }
        let digest: [u8; 32] = hash.finalize().into();
        if range.offset == 0 && end == file.length && digest != file.sha256 {
            return Err(crate::Error::backend(
                "GetFileHash",
                crate::BackendFailure::Corrupt,
                std::io::Error::other("immutable content digest mismatch"),
            ));
        }
        Ok(FileHash {
            bytes_read: end - range.offset,
            sha256: digest,
        })
    }
}
