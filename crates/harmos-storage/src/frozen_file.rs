/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Freeze An Opened Source Into One Private Immutable File.

use std::{
    fs::File,
    io::Write,
    sync::{Arc, Mutex},
};

use sha2::{Digest, Sha256};

use crate::local::io_error;
use crate::{ByteRange, Result, StorageManager};

/// A private immutable copy of an opened file range, retained independently of its source.
#[derive(Clone)]
pub struct FrozenFile {
    pub(crate) backing: Arc<Backing>,
    length: u64,
    sha256: [u8; 32],
    pub(crate) session: StorageManager<crate::ErasedCatalog>,
}

pub(crate) struct Backing {
    file: Mutex<tempfile::NamedTempFile>,
}

impl FrozenFile {
    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    /// Reads the immutable snapshot through its originating storage manager.
    /// The reader owns its file handle and remains valid after this snapshot is dropped.
    pub async fn read(&self, range: ByteRange) -> Result<crate::FileReader> {
        let file = self.opened_file()?;
        self.session.open_file_handle(file, range, Some(Arc::clone(&self.backing))).await
    }

    pub(crate) fn opened_file(&self) -> Result<File> {
        let file = self.backing.file.lock().unwrap();
        file.as_file().try_clone().map_err(|error| io_error("clone frozen file", file.path(), error))
    }
}

impl<C> StorageManager<C> {
    /// Copies a stable opened range in native bounded batches. Cancellation drops
    /// the partial copy only after any blocking writer releases its actual owners.
    /// On Windows, `source` must allow a separate read open; exclusive handles are refused.
    pub async fn freeze_opened_file(&self, source: File, range: ByteRange) -> Result<FrozenFile> {
        let backing = tokio::task::spawn_blocking(move || {
            // Keep The Name Until All Frozen Readers And Captures Release Their Owner.
            // Reopening An Unlinked Temporary Can Be Denied By Windows Filesystem Filters.
            let directory = std::env::temp_dir();
            let file =
                tempfile::NamedTempFile::new_in(&directory).map_err(|error| io_error("create frozen temporary", &directory, error))?;
            Ok::<_, crate::Error>(Arc::new(Backing { file: Mutex::new(file) }))
        })
        .await
        .map_err(task_error)??;
        let mut reader = self.read_opened_file(source, range).await?;
        let mut digest = Sha256::new();
        while let Some(batch) = reader.next().await? {
            digest.update(batch.bytes());
            let target = Arc::clone(&backing);
            tokio::task::spawn_blocking(move || {
                let mut file = target.file.lock().unwrap();
                file.write_all(batch.bytes()).map_err(|error| io_error("write frozen temporary", file.path(), error))
            })
            .await
            .map_err(task_error)??;
        }
        let completion = reader.completion()?;
        if range.limit.is_some_and(|length| length != completion.bytes) {
            return Err(crate::Error::InvalidInput {
                field: "frozen file",
                reason: "source ended before the selected prefix",
            });
        }
        reader.check_unchanged().await?;
        let session = self.erased();
        Ok(FrozenFile {
            backing,
            length: completion.bytes,
            sha256: digest.finalize().into(),
            session,
        })
    }
}

fn task_error(error: tokio::task::JoinError) -> crate::Error {
    crate::Error::backend("freeze opened file", crate::BackendFailure::Permanent, error)
}
