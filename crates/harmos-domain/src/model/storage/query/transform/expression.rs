/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Streaming Field Expressions

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::mem;

// Third Party Imports

// Local Imports
use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::query::scalar::ScalarValue;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Scale {
    pub field: FieldPath,
    pub name: String,
    pub factor: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Offset {
    pub field: FieldPath,
    pub name: String,
    pub amount: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Contains {
    pub field: FieldPath,
    pub name: String,
    pub value: ScalarValue,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for Scale {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.field.memory_size()).saturating_add(self.name.capacity())
    }
}

impl MemorySize for Offset {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>().saturating_add(self.field.memory_size()).saturating_add(self.name.capacity())
    }
}

impl MemorySize for Contains {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            .saturating_add(self.field.memory_size())
            .saturating_add(self.name.capacity())
            .saturating_add(self.value.memory_size())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
