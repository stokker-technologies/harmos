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
use crate::MemorySize;
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::{FileBatch, FileMetadata, FileReference};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(WriteFileRequest {
    metadata: Nested<FileMetadata> => 1,
});

pub struct WriteFile;

impl crate::operations::OperationDeclaration for WriteFile {
    type Request = WriteFileRequest;
    type Response = FileReference;
    type Input = crate::operations::Stream<FileBatch>;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_write_file";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Unknown;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "WriteFile"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        Some(crate::plugin::Grant::of(Self::ID, request.metadata.id.as_str().to_owned()))
    }
}

impl crate::operations::RequestFor for WriteFileRequest {
    type Operation = WriteFile;
}

/* === Type Aliases === */

/* === Structs === */

// Write One Finished File From Streamed Batches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFileRequest {
    pub metadata: FileMetadata,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for WriteFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
