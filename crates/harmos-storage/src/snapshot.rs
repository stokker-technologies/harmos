/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Pinned Historical Images For Admitted Reads

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports
use super::backend::{BackendSnapshot, ScanPage};
use super::capacity::{Attempt, Refusal};
use super::config::SNAPSHOT_BYTES;
use super::manager::{Inner, State, encoded_size};
use super::selection::Entry;
use super::{FileReference, ObjectEntry, ObjectPoint, StorageCursor, StorageManager};
use crate::{Error, Result};

/* === Constants === */

static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// An admitted read's pinned backend image. The worker opens it in order with
/// object pruning and releases it after this guard drops; closing storage
/// revokes it so later reads fail instead of observing a released image.
pub(super) struct Snapshot {
    inner: Arc<Inner>,
    id: u64,
    backend: BackendSnapshot,
}

pub(super) struct SnapshotState {
    at: StorageCursor,
    opened: Option<std::result::Result<BackendSnapshot, Arc<Error>>>,
    opening: bool,
    released: bool,
}

/* === Enums === */

pub(super) enum SnapshotAction {
    Open(u64, StorageCursor),
    Release(u64, BackendSnapshot),
}

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    /// Pin the saved image at `at`. History must be durable and not yet pruned.
    pub(super) async fn snapshot(&self, at: &StorageCursor) -> Result<Snapshot> {
        self.inner.primary.history().check(&at.history)?;
        let id = NEXT_SNAPSHOT.fetch_add(1, Ordering::Relaxed);
        {
            let mut state = self.inner.state.lock().unwrap();
            super::capacity::admitting(&state, super::capacity::Pool::Read)?;
            if !state.status.saved.covers(at)? {
                return Err(Error::InvalidInput { field: "query cursor", reason: "cursor exceeds the durable storage frontier" });
            }
            state.snapshots.insert(id, SnapshotState { at: at.clone(), opened: None, opening: false, released: false });
        }
        // The Guard Exists Before Waiting, So Cancellation Releases Registration.
        let mut guard = Snapshot { inner: Arc::clone(&self.inner), id, backend: BackendSnapshot { at: at.clone(), token: Vec::new() } };
        self.inner.notify();
        let opened = self
            .inner
            .wait(None, |state| match state.snapshots.get(&id) {
                None => Attempt::Refuse(Refusal::Failed(Error::Closing { domain: "storage" })),
                Some(held) if held.released => Attempt::Refuse(Refusal::Failed(Error::Closing { domain: "storage" })),
                Some(SnapshotState { opened: Some(result), .. }) => match result {
                    Ok(snapshot) => Attempt::Ready(snapshot.clone()),
                    Err(source) => Attempt::Refuse(Refusal::Failed(Error::Persistence { source: source.clone() })),
                },
                Some(_) if !state.status.running => Attempt::Refuse(Refusal::Failed(Error::NotReady { domain: "storage worker" })),
                Some(_) => Attempt::Wait,
            })
            .await
            .map_err(Refusal::into_error)?;
        guard.backend = opened;
        Ok(guard)
    }

    /// Revoke every snapshot and wait until the worker released them.
    pub(super) async fn release_snapshots(&self) -> Result<()> {
        {
            let mut state = self.inner.state.lock().unwrap();
            for snapshot in state.snapshots.values_mut() {
                snapshot.released = true;
            }
        }
        self.inner.notify();
        self.inner
            .wait(None, |state| {
                if state.snapshots.is_empty() {
                    Attempt::Ready(())
                } else if !state.status.running {
                    Attempt::Refuse(Refusal::Failed(Error::NotReady { domain: "storage worker" }))
                } else {
                    Attempt::Wait
                }
            })
            .await
            .map_err(Refusal::into_error)
    }
}

impl Snapshot {
    /// Read one validated scan page and its encoded size.
    pub(super) async fn scan<T: Entry>(&self, selection: &T::Selection, page: ScanPage) -> Result<(Vec<T>, usize)> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let entries = self.inner.bounded(T::scan(&*self.inner.primary, &self.backend, selection, page)).await?;
        let mut previous = page.after;
        let mut bytes = 0usize;
        for entry in &entries {
            let position = entry.position();
            self.backend.at.history.check(&position.history)?;
            if position.sequence <= previous || position.sequence > self.backend.at.sequence || !entry.matches(selection) {
                return Err(invalid("backend returned an entry outside the ordered selection"));
            }
            previous = position.sequence;
            bytes = bytes.saturating_add(encoded_size(entry)?);
        }
        if entries.len() > page.rows.get() || (entries.len() > 1 && bytes > page.bytes.get()) {
            return Err(invalid("backend page exceeds its row or byte bound"));
        }
        Ok((entries, bytes))
    }

    pub(super) async fn read_object(&self, point: &ObjectPoint) -> Result<Option<ObjectEntry>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        let object = self.inner.bounded(self.inner.primary.read_object(&self.backend, point)).await?;
        if let Some(object) = &object {
            object.version.check_position(&object.position)?;
            if object.position.sequence == 0
                || !self.backend.at.covers(&object.position)?
                || object.value.schema.id != point.schema
                || object.value.item != point.item
            {
                return Err(invalid("backend returned a different object revision"));
            }
        }
        Ok(object)
    }

    pub(super) async fn read_file(&self, file: &FileReference, offset: u64, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        let _lifecycle = self.inner.lifecycle.read().await;
        self.check()?;
        self.inner.bounded(self.inner.primary.read_file(&self.backend, file, offset, max_bytes)).await
    }

    fn check(&self) -> Result<()> {
        let state = self.inner.state.lock().unwrap();
        match state.snapshots.get(&self.id) {
            Some(held) if !held.released => Ok(()),
            _ => Err(Error::Closing { domain: "storage" }),
        }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        if let Some(held) = state.snapshots.get_mut(&self.id) {
            held.released = true;
        }
        drop(state);
        self.inner.notify();
    }
}

impl State {
    /// Whether a registered snapshot still waits to pin its image; pruning pauses.
    pub(super) fn snapshot_opening(&self) -> bool {
        self.snapshots.values().any(|held| !held.released && held.opened.is_none())
    }

    pub(super) fn snapshot_action(&mut self, allow_release: bool) -> Option<SnapshotAction> {
        let mut abandoned = Vec::new();
        let mut action = None;
        for (&id, held) in &mut self.snapshots {
            if held.released && !held.opening {
                match &held.opened {
                    Some(Ok(snapshot)) if allow_release => {
                        action = Some(SnapshotAction::Release(id, snapshot.clone()));
                        break;
                    }
                    Some(Ok(_)) => {}
                    // Never Opened Or Failed: Nothing Remains Pinned In The Backend.
                    _ => abandoned.push(id),
                }
            } else if !held.released && !held.opening && held.opened.is_none() {
                held.opening = true;
                action = Some(SnapshotAction::Open(id, held.at.clone()));
                break;
            }
        }
        for id in abandoned {
            self.snapshots.remove(&id);
        }
        action
    }

    pub(super) fn snapshot_opened(&mut self, id: u64, result: Result<BackendSnapshot>) {
        let Some(held) = self.snapshots.get_mut(&id) else {
            return;
        };
        held.opening = false;
        held.opened = Some(match result {
            Ok(snapshot) if snapshot.at != held.at || snapshot.token.len() > SNAPSHOT_BYTES => {
                Err(Arc::new(invalid("backend snapshot differs from its requested image")))
            }
            Ok(snapshot) => Ok(snapshot),
            Err(error) => Err(Arc::new(error)),
        });
    }

    pub(super) fn snapshot_removed(&mut self, id: u64) {
        self.snapshots.remove(&id);
        // A Released Image Lets The Worker Resume Pruning From The Start.
        self.objects_pruned = 0;
        self.objects_prune_owed = true;
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "read snapshot", reason }
}

/* === Tests === */
