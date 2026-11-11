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
use super::IO_POLICY;
use harmos_domain::MemorySize;
use harmos_domain::model::storage::codec::{Code, Nested, product};
use harmos_domain::model::storage::files::{ByteRange, CaptureConsistency, FileMetadata, FileReference, FileSource};
use harmos_domain::operations::ReplaySafety;

#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(CaptureFile {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
    metadata: Nested<FileMetadata> => 3,
    consistency: Code<CaptureConsistency> => 4,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.capture_file",
    response = FileReference,
    policy = IO_POLICY,
    visibility = Public,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "CaptureFile"),
    grant = Self::grant_for)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFile {
    pub source: FileSource,
    pub range: ByteRange,
    pub metadata: FileMetadata,
    pub consistency: CaptureConsistency,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl CaptureFile {
    fn grant_for(request: &CaptureFile) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

impl MemorySize for CaptureFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
            .saturating_add(self.consistency.memory_size().saturating_sub(std::mem::size_of_val(&self.consistency)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

#[cfg(feature = "runtime")]
mod execution {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::execution::operations::Operation;
    use crate::{Error, Result};
    use harmos_domain::model::storage::files::FileReference;

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    impl Operation for super::CaptureFile {
        type Error = Error;

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.capture_file", level = "trace", skip_all)]
        async fn run(self, context: crate::execution::Context<'_, Self>) -> Result<crate::execution::operations::Response<FileReference>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();

            Ok(storage
                .capture_file(harmos_storage::FileCapture {
                    source: self.source.clone(),
                    range: self.range,
                    metadata: self.metadata.clone(),
                    consistency: self.consistency,
                })
                .await?
                .into())
        }

        fn classify(error: &Error) -> crate::execution::operations::FailureKind {
            crate::storage::classify(error)
        }
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */
