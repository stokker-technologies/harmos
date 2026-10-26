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
use super::STREAM_POLICY;
use crate::MemorySize;
use crate::model::storage::codec::{Nested, Optional, product};
use crate::model::storage::{StorageCursor, StoredRow, StreamSelection};
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowStreamRequest {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<StreamSelection> => 2,
});

pub struct FollowStream;

impl crate::operations::OperationDeclaration for FollowStream {
    type Request = FollowStreamRequest;
    type Response = StorageCursor;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<StoredRow>;
    const ID: &'static str = "storage_follow_stream";
    const POLICY: crate::operations::OperationPolicy = STREAM_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
}

impl crate::operations::RequestFor for FollowStreamRequest {
    type Operation = FollowStream;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowStreamRequest {
    pub after: Option<StorageCursor>,
    pub selection: StreamSelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowStreamRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
