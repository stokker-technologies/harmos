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
use crate::model::storage::codec::{Nested, Repeated, Text, product};

/* === Constants === */

/* === Macros === */
product!(Project { fields: Repeated<Projection> => 1 });
product!(Projection { field: Nested<FieldPath> => 1, name: Text => 2 });

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct Project {
    pub fields: Vec<Projection>,
}
#[derive(Clone, Debug)]
pub struct Projection {
    pub field: FieldPath,
    pub name: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl MemorySize for Project {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.fields.memory_size().saturating_sub(std::mem::size_of_val(&self.fields))
    }
}
impl MemorySize for Projection {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.field.memory_size().saturating_sub(std::mem::size_of_val(&self.field)) + self.name.capacity()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
