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
use crate::model::storage::files::FileSource;
use crate::model::storage::{ByteRange, FileBatch, FileCompletion};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(ReadFileRequest {
    source: Nested<FileSource> => 1,
    range: Nested<ByteRange> => 2,
});

pub struct ReadFile;

impl crate::operations::OperationDeclaration for ReadFile {
    type Request = ReadFileRequest;
    type Response = FileCompletion;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<FileBatch>;
    const ID: &'static str = "storage_read_file";
    const POLICY: crate::operations::OperationPolicy = IO_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Shared;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "ReadFile"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        request.source.grant_scope().map(|scope| crate::plugin::Grant::of(Self::ID, scope))
    }
}

impl crate::operations::RequestFor for ReadFileRequest {
    type Operation = ReadFile;
}

/* === Type Aliases === */

/* === Structs === */

// Read A Bounded Byte Range Of A Stored Or Local File.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFileRequest {
    pub source: FileSource,
    pub range: ByteRange,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for ReadFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.source.memory_size().saturating_sub(std::mem::size_of_val(&self.source)))
            .saturating_add(self.range.memory_size().saturating_sub(std::mem::size_of_val(&self.range)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
