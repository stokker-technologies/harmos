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
use crate::model::storage::codec::{Nested, Repeated, Text, product};

use super::FieldPath;
use super::project::Projection;
use crate::MemorySize;

/* === Constants === */

/* === Macros === */

product!(Aggregate { groups: Repeated<Projection> => 1, measures: Repeated<Measure> => 2 });
product!(Measure { field: Nested<FieldPath> => 1, statistic: Nested<crate::Aggregate> => 2, name: Text => 3 });

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Aggregate {
    pub groups: Vec<Projection>,
    pub measures: Vec<Measure>,
}
#[derive(Clone, Debug)]
pub struct Measure {
    pub field: FieldPath,
    pub statistic: crate::Aggregate,
    pub name: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for Aggregate {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.groups.memory_size().saturating_sub(std::mem::size_of_val(&self.groups))
            + self.measures.memory_size().saturating_sub(std::mem::size_of_val(&self.measures))
    }
}
impl MemorySize for Measure {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.field.memory_size().saturating_sub(std::mem::size_of_val(&self.field)) + self.name.capacity()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
