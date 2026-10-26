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

product!(AppendFileRequest {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
    offset: Unsigned => 3,
    bytes: Bytes => 4,
});

pub struct AppendFile;

impl crate::operations::OperationDeclaration for AppendFile {
    type Request = AppendFileRequest;
    type Response = FileProducer;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_append_file";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

impl crate::operations::RequestFor for AppendFileRequest {
    type Operation = AppendFile;
}

/* === Type Aliases === */

/* === Structs === */

// Append Bytes At An Offset Of A Producing File Session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendFileRequest {
    pub mutation: MutationId,
    pub session: FileSession,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for AppendFileRequest {
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
