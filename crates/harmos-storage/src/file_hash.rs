/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Hash A Bounded File Range Without Retaining The Complete Payload.

use sha2::{Digest, Sha256};

use crate::FileRead;
use crate::{Result, StorageManager};
use harmos_domain::model::storage::files::FileHash;

impl<C> StorageManager<C> {
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
