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
use crate::model::storage::{MutationId, StagedFile};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(DeleteFileRequest {
    mutation: Nested<MutationId> => 1,
    file: Nested<StagedFile> => 2,
});

pub struct DeleteFile;

impl crate::operations::OperationDeclaration for DeleteFile {
    type Request = DeleteFileRequest;
    type Response = StagedFile;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_delete_file";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

impl crate::operations::RequestFor for DeleteFileRequest {
    type Operation = DeleteFile;
}

/* === Type Aliases === */

/* === Structs === */

// Delete A Staged File, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFileRequest {
    pub mutation: MutationId,
    pub file: StagedFile,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for DeleteFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.file.memory_size().saturating_sub(std::mem::size_of_val(&self.file)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
