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
use super::POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, product};
use crate::model::storage::{FileMetadata, FileProducer, MutationId};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(CreateFileRequest {
    mutation: Nested<MutationId> => 1,
    metadata: Nested<FileMetadata> => 2,
});

pub struct CreateFile;

impl crate::operations::OperationDeclaration for CreateFile {
    type Request = CreateFileRequest;
    type Response = FileProducer;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_create_file";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

impl crate::operations::RequestFor for CreateFileRequest {
    type Operation = CreateFile;
}

/* === Type Aliases === */

/* === Structs === */

// Open A Producing File Session For New Metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFileRequest {
    pub mutation: MutationId,
    pub metadata: FileMetadata,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for CreateFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.metadata.memory_size().saturating_sub(std::mem::size_of_val(&self.metadata)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
