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
use crate::model::storage::FileProducer;
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */
crate::model::storage::codec::product!(CreateFileRequest {
    mutation: crate::model::storage::codec::Nested<crate::model::storage::MutationId> => 1,
    metadata: crate::model::storage::codec::Nested<crate::model::storage::FileMetadata> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateFileRequest {
    pub mutation: crate::model::storage::MutationId,
    pub metadata: crate::model::storage::FileMetadata,
}

pub struct CreateFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for CreateFile {
    type Request = CreateFileRequest;
    type Response = FileProducer;
    const ID: &'static str = "storage.create_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for CreateFile {
    const VERSION: u32 = 1;
}

impl crate::MemorySize for CreateFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<crate::model::storage::MutationId>()
            + self.metadata.memory_size()
            - std::mem::size_of::<crate::model::storage::FileMetadata>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
