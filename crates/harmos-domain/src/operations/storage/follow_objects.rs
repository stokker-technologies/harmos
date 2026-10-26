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
use crate::model::storage::{ObjectEntry, ObjectSelection, StorageCursor};
use crate::operations::Visibility;

/* === Constants === */

/* === Macros === */

product!(FollowObjectsRequest {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<ObjectSelection> => 2,
});

pub struct FollowObjects;

impl crate::operations::OperationDeclaration for FollowObjects {
    type Request = FollowObjectsRequest;
    type Response = StorageCursor;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<ObjectEntry>;
    const ID: &'static str = "storage_follow_objects";
    const POLICY: crate::operations::OperationPolicy = STREAM_POLICY;
    const VISIBILITY: crate::operations::Visibility = Visibility::Host;
}

impl crate::operations::RequestFor for FollowObjectsRequest {
    type Operation = FollowObjects;
}

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowObjectsRequest {
    pub after: Option<StorageCursor>,
    pub selection: ObjectSelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowObjectsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
