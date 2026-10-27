/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// CaptureFileRequest Declaration.

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
use harmos_domain::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(CaptureFileRequest {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
    metadata: Nested<FileMetadata> => 3,
    consistency: Code<CaptureConsistency> => 4,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFileRequest {
    pub source: FileSource,
    pub range: ByteRange,
    pub metadata: FileMetadata,
    pub consistency: CaptureConsistency,
}

#[harmos::operation(harmos_path = crate,
    id = "storage_capture_file",
    request = CaptureFileRequest,
    response = FileReference,
    policy = IO_POLICY,
    infer_request,
    visibility = Visibility::Shared,
    replay = ReplaySafety::Unknown,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "CaptureFile"),
    grant = Self::grant_for)]
pub struct CaptureFile {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl CaptureFile {
    fn grant_for(request: &CaptureFileRequest) -> Option<harmos_domain::plugin::Grant> {
        request.source.grant_scope().map(|scope| harmos_domain::plugin::Grant::of(<Self as harmos_domain::OperationDeclaration>::ID, scope))
    }
}

impl MemorySize for CaptureFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
            .saturating_add(self.consistency.memory_size().saturating_sub(std::mem::size_of_val(&self.consistency)))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // CaptureFile Runtime Handler.

    // Standard Library Imports

    // Third Party Imports

    // Local Imports
    use crate::{Error, Result};
    use harmos_domain::model::storage::files::FileReference;
    use harmos_sdk::operations::Operation;
    use harmos_storage::StorageManager;

    impl Operation for super::CaptureFile {
        type Request = <CaptureFile as harmos_domain::OperationDeclaration>::Request;
        type Response = <CaptureFile as harmos_domain::OperationDeclaration>::Response;
        type Error = Error;
        type Endpoints = ();

        async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
            let storage = context.dependency::<StorageManager<harmos_storage::ErasedCatalog>>()?;
            Ok(Self { storage })
        }

        #[tracing::instrument(target = "harmos::storage", name = "storage.operation.capture_file", level = "trace", skip_all)]
        async fn run(
            &mut self, _: harmos_sdk::OperationContext, request: &mut Self::Request, _: &mut (),
        ) -> Result<harmos_sdk::operations::Response<FileReference>> {
            Ok(self
                .storage
                .capture_file(harmos_storage::FileCapture {
                    source: request.source.clone(),
                    range: request.range,
                    metadata: request.metadata.clone(),
                    consistency: request.consistency,
                })
                .await?
                .into())
        }

        fn classify(error: &Error) -> harmos_sdk::operations::FailureKind {
            crate::storage::classify(error)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
