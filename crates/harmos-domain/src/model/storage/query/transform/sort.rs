/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::codec::{Flag, Nested, Repeated, product};

product!(Sort { keys: Repeated<SortKey> => 1 });
product!(SortKey { field: Nested<FieldPath> => 1, descending: Flag => 2, nulls_first: Flag => 3 });

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
