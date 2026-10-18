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
use crate::model::storage::codec::{Bytes, Nested, Unsigned, product};
use crate::model::storage::{FileProducer, FileSession, MutationId};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */
product!(AppendFile {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
    offset: Unsigned => 3,
    bytes: Bytes => 4,
});

crate::declare!(AppendFile {
    id: "storage.append_file",
    version: 1,
    policy: POLICY,
    response: FileProducer,
    visibility: Visibility::Host,
    replay: ReplaySafety::Idempotent,
});

/* === Type Aliases === */

/* === Structs === */
// Append Bytes At An Offset Of A Producing File Session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFile {
    pub mutation: MutationId,
    pub session: FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for AppendFile {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
            .saturating_add(self.offset.memory_size().saturating_sub(std::mem::size_of_val(&self.offset)))
            .saturating_add(self.bytes.memory_size().saturating_sub(std::mem::size_of_val(&self.bytes)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
