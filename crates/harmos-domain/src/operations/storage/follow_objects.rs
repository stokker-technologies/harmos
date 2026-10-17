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
product!(FollowObjects {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<ObjectSelection> => 2,
});

crate::declare!(FollowObjects {
    id: "storage.follow_objects",
    version: 1,
    policy: STREAM_POLICY,
    response: StorageCursor,
    output: ObjectEntry,
    visibility: Visibility::Host,
});

/* === Type Aliases === */

/* === Structs === */
/// Follow every object revision accepted after `after` (from the start when absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowObjects {
    pub after: Option<StorageCursor>,
    pub selection: ObjectSelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for FollowObjects {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
