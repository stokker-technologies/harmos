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

product!(FollowStream {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<StreamSelection> => 2,
});

crate::declare!(FollowStream {
    id: "storage.follow_stream",
    version: 1,
    policy: STREAM_POLICY,
    response: StorageCursor,
    output: StoredRow,
    visibility: Visibility::Host,
});

/* === Type Aliases === */

/* === Structs === */

/// Follow stream rows accepted after both `after` (from the start when absent)
/// and `selection.after`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowStream {
    pub after: Option<StorageCursor>,
    pub selection: StreamSelection,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FollowStream {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
