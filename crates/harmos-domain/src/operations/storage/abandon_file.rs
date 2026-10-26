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
use crate::model::storage::{FileProgress, FileSession, MutationId};
use crate::operations::{ReplaySafety, Visibility};

/* === Constants === */

/* === Macros === */

product!(AbandonFileRequest {
    mutation: Nested<MutationId> => 1,
    session: Nested<FileSession> => 2,
});

pub struct AbandonFile;

impl crate::operations::OperationDeclaration for AbandonFile {
    type Request = AbandonFileRequest;
    type Response = FileProgress;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::NoStream;
    const ID: &'static str = "storage_abandon_file";
    const POLICY: crate::operations::OperationPolicy = POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
    const REPLAY: crate::operations::ReplaySafety = ReplaySafety::Idempotent;
}

impl crate::operations::RequestFor for AbandonFileRequest {
    type Operation = AbandonFile;
}

/* === Type Aliases === */

/* === Structs === */

// Abandon A Producing File Session, Exactly Once Per Mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbandonFileRequest {
    pub mutation: MutationId,
    pub session: FileSession,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for AbandonFileRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.mutation.memory_size().saturating_sub(std::mem::size_of_val(&self.mutation)))
            .saturating_add(self.session.memory_size().saturating_sub(std::mem::size_of_val(&self.session)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
