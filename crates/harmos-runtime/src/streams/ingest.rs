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
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::watch;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The cursor each live feed has taken in. A plugin publication never evicts
// a row past the slowest feed, and a sidecar publication waits for it.
pub(crate) struct Ingest {
    taken: watch::Sender<BTreeMap<u64, u64>>,
    next: AtomicU64,
    work: Mutex<BTreeMap<u64, crate::clock::Activity>>,
}

#[derive(Debug)]
pub(crate) struct Intake {
    ingest: Arc<Ingest>,
    id: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl std::fmt::Debug for Ingest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Ingest").field("taken", &self.taken).finish_non_exhaustive()
    }
}

impl Ingest {
    pub(crate) fn track(&self, cursor: u64, activity: crate::clock::Activity) {
        self.work.lock().expect("stream work lock is healthy").insert(cursor, activity);
    }

    pub(crate) fn drained(&self, cursor: u64) {
        self.work.lock().expect("stream work lock is healthy").retain(|pending, _| *pending > cursor);
    }

    pub(crate) fn new() -> Self {
        Self {
            taken: watch::channel(BTreeMap::new()).0,
            next: AtomicU64::new(0),
            work: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn engage(self: &Arc<Self>, resume: u64) -> Intake {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.taken.send_modify(|taken| {
            taken.insert(id, resume);
        });
        Intake { ingest: Arc::clone(self), id }
    }

    // Every row at or below this cursor is in every live feed's hands.
    pub(crate) fn taken(&self) -> u64 {
        self.taken.borrow().values().min().copied().unwrap_or(u64::MAX)
    }

    pub(crate) fn watch(&self) -> watch::Receiver<BTreeMap<u64, u64>> {
        self.taken.subscribe()
    }

    // Revocation and close re-evaluate every waiting publication.
    pub(crate) fn wake(&self) {
        self.taken.send_modify(|_| {});
    }
}

impl Intake {
    pub(crate) fn took(&self, cursor: u64) {
        self.ingest.taken.send_if_modified(|taken| {
            let entry = taken.entry(self.id).or_default();
            let advanced = *entry < cursor;
            *entry = (*entry).max(cursor);
            advanced
        });
    }
}

impl Drop for Intake {
    fn drop(&mut self) {
        self.ingest.taken.send_modify(|taken| {
            taken.remove(&self.id);
        });
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
