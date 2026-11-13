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
use super::FieldPath;
use crate::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct Downsample {
    pub max_rows: usize,
    pub algorithm: Algorithm,
    pub value: Option<FieldPath>,
    pub axis: Option<FieldPath>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Decimate,
    Extrema,
    Lttb,
    LttbLog,
}

/* === Traits === */

/* === Implementations === */

impl MemorySize for Downsample {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.value.memory_size().saturating_sub(std::mem::size_of_val(&self.value))
            + self.axis.memory_size().saturating_sub(std::mem::size_of_val(&self.axis))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
