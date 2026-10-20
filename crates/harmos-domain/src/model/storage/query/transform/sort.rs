/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::codec::{Flag, Nested, Repeated, product};

/* === Constants === */

/* === Macros === */

product!(Sort { keys: Repeated<SortKey> => 1 });
product!(SortKey { field: Nested<FieldPath> => 1, descending: Flag => 2, nulls_first: Flag => 3 });

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Sort {
    pub keys: Vec<SortKey>,
}
#[derive(Clone, Debug)]
pub struct SortKey {
    pub field: FieldPath,
    pub descending: bool,
    pub nulls_first: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for Sort {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.keys.memory_size().saturating_sub(std::mem::size_of_val(&self.keys))
    }
}
impl MemorySize for SortKey {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.field.memory_size().saturating_sub(std::mem::size_of_val(&self.field))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
