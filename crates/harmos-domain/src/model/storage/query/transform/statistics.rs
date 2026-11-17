/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Statistics Contracts and Execution

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::mem;
use std::num::{NonZeroU64, NonZeroUsize};

// Third Party Imports

// Local Imports
use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::StorageCursor;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Exact Quartiles and Tukey Whiskers; Outliers Are Selected In Source Order.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BoxSummary {
    pub field: FieldPath,
    pub max_outliers: usize,
    pub memory_bytes: usize,
    pub scratch_bytes: usize,
}

/// A Bounded Envelope Over Append-Only Samples At One Source Cut.
/// Supply The Previous Result's `continuation` Bytes To Extend It At A New Cut.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BucketSummary {
    pub time: FieldPath,
    pub position: FieldPath,
    pub value: FieldPath,
    pub width_ns: NonZeroU64,
    pub max_buckets: NonZeroUsize,
    pub source_cut: StorageCursor,
    pub prior: Option<Vec<u8>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for BoxSummary {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>() + self.field.memory_size().saturating_sub(mem::size_of_val(&self.field))
    }
}
impl MemorySize for BucketSummary {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>()
            + self.time.memory_size().saturating_sub(mem::size_of_val(&self.time))
            + self.position.memory_size().saturating_sub(mem::size_of_val(&self.position))
            + self.value.memory_size().saturating_sub(mem::size_of_val(&self.value))
            + self.source_cut.memory_size().saturating_sub(mem::size_of_val(&self.source_cut))
            + self.prior.as_ref().map_or(0, Vec::capacity)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
