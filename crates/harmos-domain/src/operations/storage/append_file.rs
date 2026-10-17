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
crate::model::storage::codec::product!(AppendFileRequest {
    mutation: crate::model::storage::codec::Nested<crate::model::storage::MutationId> => 1,
    session: crate::model::storage::codec::Nested<crate::model::storage::FileSession> => 2,
    offset: crate::model::storage::codec::Unsigned => 3,
    bytes: crate::model::storage::codec::Bytes => 4,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFileRequest {
    pub mutation: crate::model::storage::MutationId,
    pub session: crate::model::storage::FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

pub struct AppendFile;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for AppendFile {
    type Request = AppendFileRequest;
    type Response = FileProducer;
    const ID: &'static str = "storage.append_file";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for AppendFile {
    const VERSION: u32 = 1;
}

impl crate::MemorySize for AppendFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<crate::model::storage::MutationId>()
            + self.session.memory_size()
            - std::mem::size_of::<crate::model::storage::FileSession>()
            + self.offset.memory_size()
            - std::mem::size_of::<u64>()
            + self.bytes.memory_size()
            - std::mem::size_of::<Vec<u8>>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
