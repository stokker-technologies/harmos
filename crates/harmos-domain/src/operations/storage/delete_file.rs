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
use crate::model::storage::{MutationId, StagedFile};
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */
crate::model::storage::codec::product!(DeleteFileRequest {
    mutation: crate::model::storage::codec::Nested<MutationId> => 1,
    file: crate::model::storage::codec::Nested<StagedFile> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteFileRequest {
    pub mutation: MutationId,
    pub file: StagedFile,
}

pub struct DeleteFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::MemorySize for DeleteFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<MutationId>()
            + self.file.memory_size()
            - std::mem::size_of::<StagedFile>()
    }
}

impl OperationDeclaration for DeleteFile {
    type Request = DeleteFileRequest;
    type Response = StagedFile;
    const ID: &'static str = "storage.delete_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for DeleteFile {
    const VERSION: u32 = 1;
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
