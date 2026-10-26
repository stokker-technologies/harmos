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
use crate::MemorySize;
use crate::model::storage::codec::{Code, Nested, product};
use crate::model::storage::files::{ByteRange, CaptureConsistency, FileMetadata, FileReference, FileSource};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(CaptureFileRequest {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
    metadata: Nested<FileMetadata> => 3,
    consistency: Code<CaptureConsistency> => 4,
});

pub struct CaptureFile;

impl crate::operations::OperationDeclaration for CaptureFile {
    type Request = CaptureFileRequest;
    type Response = FileReference;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_capture_file";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Unknown;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "CaptureFile"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        request.source.grant_scope().map(|scope| crate::plugin::Grant::of("storage.capture_file", scope))
    }
}

impl crate::operations::RequestFor for CaptureFileRequest {
    type Operation = CaptureFile;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFileRequest {
    pub source: FileSource,
    pub range: ByteRange,
    pub metadata: FileMetadata,
    pub consistency: CaptureConsistency,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for CaptureFileRequest {
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

/* === Tests === */
