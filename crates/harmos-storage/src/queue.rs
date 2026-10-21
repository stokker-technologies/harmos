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
use std::collections::VecDeque;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::manager::{Domain, Pending};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub(super) struct PendingQueue {
    entries: VecDeque<Arc<Pending>>,
    bytes: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PendingQueue {
    pub(super) fn push_back(&mut self, entry: Arc<Pending>) {
        self.bytes += entry.bytes;
        self.entries.push_back(entry);
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Arc<Pending>> {
        self.entries.iter()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn retire(&mut self, domain: Domain, through: u64) {
        self.retire_through(domain, through, u64::MAX);
    }

    pub(super) fn retire_data(&mut self, through: u64, sequence: u64) {
        self.retire_through(Domain::Data, through, sequence);
    }

    fn retire_through(&mut self, domain: Domain, through: u64, sequence: u64) {
        let bytes = &mut self.bytes;
        self.entries.retain(|entry| {
            if entry.index <= through
                && entry.operation.domain() == domain
                && (sequence == u64::MAX || entry.operation.sequence() <= sequence)
            {
                *bytes -= entry.bytes;
                false
            } else {
                true
            }
        });
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
