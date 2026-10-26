/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Live Storage Subscriptions

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// Third Party Imports

// Local Imports
use super::backend::{ReadLimits, ScanPage};
use super::capacity::{Attempt, Held, Pool, Refusal, Reservation};
use super::config::SCAN_ROWS;
use super::manager::{Domain, ENTRY_OVERHEAD, Inner, State, encoded_size};
pub use super::selection::Entry;
use super::selection::Queued;
use super::snapshot::Snapshot;
use super::{StorageCursor, StorageManager};
use crate::{Error, Result};

/* === Constants === */

static NEXT_FOLLOWER: AtomicU64 = AtomicU64::new(1);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Follower<T: Entry> {
    inner: Arc<Inner>,
    id: u64,
    held: Held,
    selection: T::Selection,
    history: Option<Snapshot>,
    after: u64,
    saved: StorageCursor,
    accepted: StorageCursor,
}

pub struct FollowPage<T> {
    entries: Vec<T>,
    _charge: crate::arrow::RetainedMemory,
}

pub(super) struct Following {
    pub(super) after: u64,
    failure: Option<Failure>,
}

/* === Enums === */

#[derive(Clone, Copy)]
enum Failure {
    Gap { requested: u64, available: u64 },
    Closed,
}

/* === Traits === */

/* === Implementations === */

impl<C> StorageManager<C> {
    pub async fn follow<T: Entry>(&self, after: Option<&StorageCursor>, selection: T::Selection) -> Result<Follower<T>> {
        if let Some(after) = after {
            self.inner.primary.history().check(&after.history)?;
        }
        let requested = after.map_or(0, |after| after.sequence);
        let after = requested.max(T::start(&selection));
        let id = NEXT_FOLLOWER.fetch_add(1, Ordering::Relaxed);

        // Register Queue Retention From The Saved Frontier Atomically.
        let (saved, accepted) = {
            let mut state = self.inner.state.lock().unwrap();
            super::capacity::admitting(&state, Pool::Read)?;
            let saved = state.status.saved.clone();
            let accepted = state.status.accepted.clone();
            if requested > accepted.sequence {
                return Err(Error::InvalidInput { field: "storage follow", reason: "start exceeds accepted boundary" });
            }
            state.followers.insert(id, Following { after: saved.sequence.max(after), failure: None });
            (saved, accepted)
        };
        let mut follower =
            Follower { inner: Arc::clone(&self.inner), id, held: Held::default(), selection, history: None, after, saved, accepted };
        if after < follower.saved.sequence {
            follower.history = Some(self.snapshot(&follower.saved, &follower.held).await?);
        }
        Ok(follower)
    }
}

impl<C> StorageManager<C> {
    pub async fn page<T: Entry>(&self, selection: &T::Selection, after: u64, limits: ReadLimits) -> Result<FollowPage<T>> {
        let held = Held::default();
        let rows = limits.rows.min(SCAN_ROWS);
        let bytes = limits.bytes.min(self.inner.settings.scan_bytes());
        let charge = rows.get().saturating_mul(ENTRY_OVERHEAD).saturating_add(bytes.get());
        let reservation = Reservation::wait(&self.inner, Pool::Read, &held, charge, None).await.map_err(Refusal::into_error)?.retain();
        let snapshot = self.snapshot(&self.cursor_saved(), &held).await?;
        let (entries, encoded) = snapshot.scan::<T>(selection, ScanPage { after, rows, bytes }, reservation.clone()).await?;
        reservation.shrink_to(encoded.saturating_add(entries.len().saturating_mul(ENTRY_OVERHEAD)))?;
        Ok(FollowPage { entries, _charge: reservation })
    }
}

impl<T: Entry> Follower<T> {
    pub fn saved_boundary(&self) -> &StorageCursor {
        &self.saved
    }

    pub fn accepted_boundary(&self) -> &StorageCursor {
        &self.accepted
    }

    pub async fn next(&mut self) -> Result<FollowPage<T>> {
        loop {
            if let Some(page) = self.next_history().await? {
                return Ok(page);
            }
            if let Some(page) = self.next_live().await? {
                return Ok(page);
            }
        }
    }

    // Saved History Through The Registration Frontier, From The Pinned Image.
    async fn next_history(&mut self) -> Result<Option<FollowPage<T>>> {
        let Some(snapshot) = &self.history else {
            return Ok(None);
        };
        let bytes = self.inner.settings.scan_bytes();
        let reservation =
            Reservation::wait(&self.inner, Pool::Read, &self.held, page_charge(bytes), None).await.map_err(Refusal::into_error)?.retain();
        let page = ScanPage { after: self.after, rows: SCAN_ROWS, bytes };
        let (entries, encoded) = snapshot.scan::<T>(&self.selection, page, reservation.clone()).await?;
        let Some(last) = entries.last() else {
            self.history = None;
            self.after = self.after.max(self.saved.sequence);
            return Ok(None);
        };
        self.after = last.position().sequence;
        reservation.shrink_to(encoded.saturating_add(entries.len().saturating_mul(ENTRY_OVERHEAD)))?;
        Ok(Some(FollowPage { entries, _charge: reservation }))
    }

    // Accepted Entries From The Queue; Waits Without Allocating While None Match.
    async fn next_live(&mut self) -> Result<Option<FollowPage<T>>> {
        let id = self.id;
        let selection = &self.selection;
        self.inner
            .wait(None, |state| match find::<T>(state, id, selection) {
                Ok(true) => Attempt::Ready(()),
                Ok(false) => Attempt::Wait,
                Err(error) => Attempt::Refuse(Refusal::Failed(error)),
            })
            .await
            .map_err(Refusal::into_error)?;

        // Charge The Page Before Copying Entries Out Of The Queue.
        let bytes = self.inner.settings.scan_bytes();
        let mut reservation =
            Reservation::wait(&self.inner, Pool::Read, &self.held, page_charge(bytes), None).await.map_err(Refusal::into_error)?;
        let (entries, encoded) = {
            let mut state = self.inner.state.lock().unwrap();
            copy::<T>(&mut state, self.id, &self.selection, bytes)?
        };
        let Some(last) = entries.last() else {
            return Ok(None);
        };
        self.after = last.position().sequence;
        reservation.shrink_to(encoded.saturating_add(entries.len().saturating_mul(ENTRY_OVERHEAD)));
        Ok(Some(FollowPage { entries, _charge: reservation.retain() }))
    }
}

impl<T: Entry> Drop for Follower<T> {
    fn drop(&mut self) {
        let mut state = self.inner.state.lock().unwrap();
        state.followers.remove(&self.id);
        state.retire_data();
        state.collect();
        drop(state);
        self.inner.notify();
    }
}

impl<T> FollowPage<T> {
    pub fn entries(&self) -> &[T] {
        &self.entries
    }
}

impl Following {
    fn retains(&self) -> bool {
        self.failure.is_none()
    }

    fn check(&self) -> Result<()> {
        match self.failure {
            Some(Failure::Gap { requested, available }) => Err(Error::Gap { requested, available }),
            Some(Failure::Closed) => Err(Error::Closing { domain: "storage" }),
            None => Ok(()),
        }
    }
}

impl State {
    pub(super) fn close_followers(&mut self) {
        for following in self.followers.values_mut() {
            following.failure.get_or_insert(Failure::Closed);
        }
        self.retire_data();
    }

    pub(super) fn retire_data(&mut self) {
        let through =
            self.followers.values().filter(|following| following.retains()).map(|following| following.after).min().unwrap_or(u64::MAX);
        self.queue.retire_data(self.data_flushed, through);
    }

    pub(super) fn lag_followers(&mut self) -> bool {
        let Some(through) = self
            .queue
            .iter()
            .filter(|entry| entry.operation.domain() == Domain::Data && entry.index <= self.data_flushed)
            .map(|entry| entry.operation.sequence())
            .max()
        else {
            return false;
        };
        let mut changed = false;
        for following in self.followers.values_mut() {
            if following.retains() && following.after < through {
                following.failure =
                    Some(Failure::Gap { requested: following.after.saturating_add(1), available: through.saturating_add(1) });
                changed = true;
            }
        }
        if changed {
            self.retire_data();
            self.collect();
        }
        changed
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn page_charge(bytes: NonZeroUsize) -> usize {
    SCAN_ROWS.get().saturating_mul(ENTRY_OVERHEAD).saturating_add(bytes.get())
}

// Whether A Matching Queued Entry Follows The Subscription Cursor. Queue
// Entries Without A Match Advance The Cursor So They Can Be Retired.
fn find<T: Entry>(state: &mut State, id: u64, selection: &T::Selection) -> Result<bool> {
    super::capacity::admitting(state, Pool::Read)?;
    let following = state.followers.get(&id).ok_or(Error::Closing { domain: "storage follow" })?;
    following.check()?;
    let mut cursor = following.after;
    let mut found = false;
    for pending in state.queue.iter().filter(|entry| entry.operation.domain() == Domain::Data) {
        if pending.operation.sequence() <= cursor {
            continue;
        }
        let entries = T::pending(Queued::new(&pending.operation));
        let start = entries.partition_point(|entry| entry.position().sequence <= cursor);
        if entries[start..].iter().any(|entry| entry.matches(selection)) {
            found = true;
            break;
        }
        cursor = pending.operation.sequence();
    }
    let following = state.followers.get_mut(&id).expect("registered follower");
    if cursor > following.after {
        following.after = cursor;
        state.retire_data();
        state.collect();
    }
    Ok(found)
}

// Copy Matching Queued Entries After The Cursor Within The Page Bounds.
fn copy<T: Entry>(state: &mut State, id: u64, selection: &T::Selection, bytes: NonZeroUsize) -> Result<(Vec<T>, usize)> {
    let following = state.followers.get(&id).ok_or(Error::Closing { domain: "storage follow" })?;
    following.check()?;
    let mut cursor = following.after;
    let mut entries = Vec::new();
    let mut encoded = 0usize;
    'queue: for pending in state.queue.iter().filter(|entry| entry.operation.domain() == Domain::Data) {
        if pending.operation.sequence() <= cursor {
            continue;
        }
        let values = T::pending(Queued::new(&pending.operation));
        let start = values.partition_point(|entry| entry.position().sequence <= cursor);
        for entry in &values[start..] {
            if entries.len() >= SCAN_ROWS.get() {
                break 'queue;
            }
            if entry.matches(selection) {
                let size = encoded_size(entry)?;
                if !entries.is_empty() && encoded.saturating_add(size) > bytes.get() {
                    break 'queue;
                }
                encoded = encoded.saturating_add(size);
                entries.push(entry.clone());
            }
            cursor = entry.position().sequence;
        }
        cursor = cursor.max(pending.operation.sequence());
    }
    let following = state.followers.get_mut(&id).expect("registered follower");
    following.after = following.after.max(cursor);
    state.retire_data();
    state.collect();
    Ok((entries, encoded))
}

/* === Tests === */
