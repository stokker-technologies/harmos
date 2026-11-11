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
use schematic_domain::model::transfer::Batch;

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
