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
#[cfg(feature = "runtime")]
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use crate::MemorySize;
use crate::storage::StagedFile;
#[cfg(feature = "runtime")]
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */
crate::storage_codec::product!(FileSelection {
    file: crate::storage_codec::Nested<StagedFile> => 1,
    offset: crate::storage_codec::Unsigned => 2,
    chunk_bytes: crate::storage_codec::Unsigned => 3,
});
crate::storage_codec::product!(FileBytes {
    offset: crate::storage_codec::Unsigned => 1,
    bytes: crate::storage_codec::Bytes => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSelection {
    pub file: StagedFile,
    pub offset: u64,
    pub chunk_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileBytes {
    pub offset: u64,
    pub bytes: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl FileSelection {
    #[cfg(feature = "runtime")]
    pub(super) fn maximum(&self) -> Result<NonZeroUsize> {
        usize::try_from(self.chunk_bytes)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or(Error::InvalidInput {
                field: "file chunk size",
                reason: "must be positive and fit the native address space",
            })
    }

    #[cfg(feature = "runtime")]
    pub(super) async fn transfer<C>(
        &self,
        storage: &crate::storage::StorageManager<C>,
        output: &mut crate::operations::StreamOutput<FileBytes>,
        follow: bool,
    ) -> Result<crate::storage::FileProgress> {
        let maximum = self.maximum()?;
        let mut reader = storage.follow_file(&self.file, self.offset).await?;
        let boundary = reader.progress();
        loop {
            let offset = reader.offset();
            let size = if follow {
                maximum
            } else {
                if offset == boundary.length {
                    return Ok(boundary);
                }
                NonZeroUsize::new(
                    maximum
                        .get()
                        .min(usize::try_from(boundary.length - offset).unwrap_or(usize::MAX)),
                )
                .expect("remaining prefix")
            };
            let Some(chunk) = reader.next_chunk(size).await? else {
                return Ok(reader.progress());
            };
            let charge = std::mem::size_of::<FileBytes>().saturating_add(chunk.len());
            output
                .send_with(charge, || {
                    Ok(FileBytes {
                        offset,
                        bytes: chunk.to_vec(),
                    })
                })
                .await?;
        }
    }
}

impl MemorySize for FileSelection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.file
                .memory_size()
                .saturating_sub(std::mem::size_of::<StagedFile>()),
        )
    }
}
impl MemorySize for FileBytes {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.bytes.capacity())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
