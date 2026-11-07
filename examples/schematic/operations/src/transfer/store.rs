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

use schematic_domain::model::transfer::Batch;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

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
