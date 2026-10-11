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
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::definition::{Table, Wired};
use super::{ManagedPublication, StreamPublicationError};
use crate::Position;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct StagedPublication {
    pub(super) publication: ManagedPublication,
    pub(super) committed: watch::Receiver<bool>,
}

pub(crate) struct Activation {
    table: Weak<Table>,
    state: Mutex<State>,
    leases: OnceLock<Vec<Arc<super::PublicationLease>>>,
}

#[derive(Default)]
struct State {
    rows: Vec<Pending>,
    committed: bool,
    evicted: Vec<super::window::Row>,
}

#[derive(Debug)]
struct Pending {
    wired: Weak<Wired>,
    position: Position,
    publication: ManagedPublication,
    reply: watch::Sender<bool>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl std::fmt::Debug for Activation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Activation")
            .field("state", &self.state)
            .field("leases", &self.leases().len())
            .finish()
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("State")
            .field("rows", &self.rows.len())
            .field("committed", &self.committed)
            .finish()
    }
}

impl StagedPublication {
    pub async fn committed(mut self) -> Result<ManagedPublication, StreamPublicationError> {
        loop {
            if *self.committed.borrow_and_update() {
                return Ok(self.publication);
            }
            self.committed
                .changed()
                .await
                .map_err(|_| StreamPublicationError::Revoked)?;
        }
    }
}

impl Activation {
    pub(crate) fn new(table: &Arc<Table>) -> Self {
        Self {
            table: Arc::downgrade(table),
            state: Mutex::new(State::default()),
            leases: OnceLock::new(),
        }
    }

    pub(super) fn retain_leases(&self, leases: Vec<Arc<super::PublicationLease>>) {
        assert!(
            self.leases.set(leases).is_ok(),
            "one original stream lease inventory"
        );
    }

    pub(crate) fn leases(&self) -> &[Arc<super::PublicationLease>] {
        self.leases.get().map_or(&[], Vec::as_slice)
    }

    pub(crate) fn visibility(&self) -> Option<Arc<RwLock<()>>> {
        self.table.upgrade().map(|table| table.visibility.clone())
    }

    pub(super) fn push(
        &self,
        wired: &Arc<Wired>,
        position: Position,
        publication: ManagedPublication,
    ) -> StagedPublication {
        let (reply, committed) = watch::channel(false);
        self.state
            .lock()
            .expect("publication staging poisoned")
            .rows
            .push(Pending {
                wired: Arc::downgrade(wired),
                position,
                publication: publication.clone(),
                reply,
            });
        StagedPublication {
            publication,
            committed,
        }
    }

    // Caller owns the table visibility write fence and original scope phase.
    pub(crate) fn commit_locked(&self) -> bool {
        let Some(table) = self.table.upgrade() else {
            return false;
        };
        let mut state = self.state.lock().expect("publication staging poisoned");
        if state.committed {
            return true;
        }
        let windows = table.windows.read().expect("stream registry poisoned");
        let mut added = 0u64;
        for pending in &state.rows {
            let Some(wired) = pending.wired.upgrade() else {
                return false;
            };
            if !windows
                .get(&wired.definition.id)
                .and_then(|revisions| revisions.get(&wired.definition.version))
                .is_some_and(|current| Arc::ptr_eq(current, &wired))
            {
                return false;
            }
            let Some(next) = added
                .checked_add(pending.publication.missed())
                .and_then(|next| next.checked_add(1))
            else {
                return false;
            };
            added = next;
        }
        if !table.can_advance(added) {
            return false;
        }
        // Per-stream cursors cannot exceed the original global cursor space.
        // The fence excludes every other publisher until this full run is installed.
        let mut evicted = Vec::new();
        for pending in &state.rows {
            let wired = pending.wired.upgrade().expect("retained staged inventory");
            let previous = wired
                .window
                .append_locked(
                    pending.position,
                    Arc::new(pending.publication.clone()),
                    pending.publication.missed(),
                )
                .expect("validated complete cursor span");
            evicted.extend(previous);
        }
        state.evicted = evicted;
        state.committed = true;
        true
    }

    pub(crate) fn complete(&self) {
        let mut state = self.state.lock().expect("publication staging poisoned");
        if !state.committed {
            return;
        }
        let rows = std::mem::take(&mut state.rows);
        let evicted = std::mem::take(&mut state.evicted);
        drop(state);
        drop(evicted);
        for pending in rows {
            pending.reply.send_replace(true);
            if let Some(wired) = pending.wired.upgrade() {
                wired.window.knock();
            }
        }
        if let Some(table) = self.table.upgrade() {
            table.pump.send_modify(|()| {});
        }
    }

    pub(crate) fn abort(&self) {
        let mut state = self.state.lock().expect("publication staging poisoned");
        if state.committed {
            return;
        }
        let rows = std::mem::take(&mut state.rows);
        drop(state);
        drop(rows);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
