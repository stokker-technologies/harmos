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
use crate::model::storage::codec::{Code, Index, Nested, Optional, coded, product};

/* === Constants === */

/* === Macros === */

product!(Downsample { max_rows: Index => 1, algorithm: Code<Algorithm> => 2, value: Optional<Nested<FieldPath>> => 3, axis: Optional<Nested<FieldPath>> => 4 });
coded!(Algorithm { Decimate, Extrema, Lttb, LttbLog });

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
