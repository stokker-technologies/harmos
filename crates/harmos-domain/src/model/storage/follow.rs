/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Live Storage Subscriptions

// Follow Requests: A Selection And The Cursor To Resume After.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::codec::{Nested, Optional, Unsigned, product};
use super::{ObjectSelection, RecordSelection, StagedFile, StorageCursor, StreamSelection};
use crate::MemorySize;

/* === Constants === */

/* === Macros === */
product!(Records {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<RecordSelection> => 2,
});
product!(Stream {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<StreamSelection> => 2,
});
product!(Objects {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<ObjectSelection> => 2,
});
product!(File {
    file: Nested<StagedFile> => 1,
    offset: Unsigned => 2,
});

/* === Type Aliases === */

/* === Structs === */
/// Follow record entries accepted after `after` (from the start when absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Records {
    pub after: Option<StorageCursor>,
    pub selection: RecordSelection,
}

/// Follow stream rows accepted after both `after` (from the start when absent)
/// and `selection.after`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    pub after: Option<StorageCursor>,
    pub selection: StreamSelection,
}

/// Follow every object revision accepted after `after` (from the start when absent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Objects {
    pub after: Option<StorageCursor>,
    pub selection: ObjectSelection,
}

/// Follow the bytes of one producing file from `offset`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub file: StagedFile,
    pub offset: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for Records {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.after))
            .saturating_add(
                self.selection
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.selection)),
            )
    }
}

impl MemorySize for Stream {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.after))
            .saturating_add(
                self.selection
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.selection)),
            )
    }
}

impl MemorySize for Objects {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.after))
            .saturating_add(
                self.selection
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(&self.selection)),
            )
    }
}

impl MemorySize for File {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.file
                .memory_size()
                .saturating_sub(std::mem::size_of::<StagedFile>()),
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn heap(cursor: &Option<StorageCursor>) -> usize {
    cursor.as_ref().map_or(0, |cursor| {
        cursor
            .memory_size()
            .saturating_sub(std::mem::size_of::<StorageCursor>())
    })
}

/* === Tests === */
