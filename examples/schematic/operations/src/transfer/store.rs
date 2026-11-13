/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin-Owned Bounded Transfer Storage.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

// Third Party Imports
use crate::protocol::Batch;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub struct Store {
    pub(super) batches: Mutex<BTreeMap<u64, Stored>>,
}

#[derive(Default)]
pub(super) struct Stored {
    pub(super) bytes: usize,
    pub(super) batches: VecDeque<Batch>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

pub(super) fn memory(batch: &Batch) -> usize {
    std::mem::size_of::<Batch>()
        .saturating_add(batch.records.capacity().saturating_mul(std::mem::size_of::<crate::protocol::Record>()))
        .saturating_add(batch.records.iter().map(|record| record.payload.capacity()).sum::<usize>())
        .saturating_add(batch.samples.capacity().saturating_mul(std::mem::size_of::<crate::protocol::Sample>()))
        .saturating_add(batch.bytes.capacity())
}
