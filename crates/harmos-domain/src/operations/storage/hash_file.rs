/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// HashFileRequest Declaration.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::IO_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::files::{ByteRange, FileHash, FileSource};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(HashFileRequest {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

pub struct HashFile;

impl crate::operations::OperationDeclaration for HashFile {
    type Request = HashFileRequest;
    type Response = FileHash;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_hash_file";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "HashFile"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        request.source.grant_scope().map(|scope| crate::plugin::Grant::of("storage.hash_file", scope))
    }
}

impl crate::operations::RequestFor for HashFileRequest {
    type Operation = HashFile;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashFileRequest {
    pub source: FileSource,
    pub range: ByteRange,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for HashFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
