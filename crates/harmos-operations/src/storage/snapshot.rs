/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retained Saved Cuts Shared by Bounded Page Calls.

/* === Imports === */

use harmos_protocol::storage as wire;
use harmos_storage::{Snapshot, StorageCursor};
use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

/* === Structs === */

pub struct SavedSnapshots {
    capacity: Arc<tokio::sync::Semaphore>,
    entries: Mutex<BTreeMap<Vec<u8>, Arc<HeldSnapshot>>>,
}

pub(crate) struct SnapshotRead<'a> {
    owner: &'a SavedSnapshots,
    handle: wire::SavedSnapshot,
    snapshot: Arc<HeldSnapshot>,
    retained: bool,
}

struct HeldSnapshot {
    snapshot: Snapshot,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

/* === Implementations === */

impl Drop for SnapshotRead<'_> {
    fn drop(&mut self) {
        if !self.retained {
            let _ = self.owner.release(&self.handle);
        }
    }
}
impl std::ops::Deref for SnapshotRead<'_> {
    type Target = Snapshot;
    fn deref(&self) -> &Self::Target {
        &self.snapshot.snapshot
    }
}
impl SnapshotRead<'_> {
    pub(crate) fn retain(&mut self) {
        self.retained = true;
    }
}

impl SavedSnapshots {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity: Arc::new(tokio::sync::Semaphore::new(capacity.get())),
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn reserve(&self) -> harmos_storage::Result<tokio::sync::OwnedSemaphorePermit> {
        self.capacity.clone().try_acquire_owned().map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "saved snapshots" })
    }
    pub(crate) fn insert(
        &self, snapshot: Snapshot, permit: tokio::sync::OwnedSemaphorePermit,
    ) -> harmos_storage::Result<wire::SavedSnapshot> {
        let mut token = vec![0; 32];
        getrandom::fill(&mut token).map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "snapshot entropy" })?;
        let mut entries = self.entries.lock().unwrap();
        let at = Some(snapshot.at().clone().into());
        entries.insert(token.clone(), Arc::new(HeldSnapshot { snapshot, _permit: permit }));
        Ok(wire::SavedSnapshot { token, at })
    }

    fn get(&self, handle: &wire::SavedSnapshot) -> harmos_storage::Result<Arc<HeldSnapshot>> {
        let at: StorageCursor = harmos_storage::query::protobuf::required(handle.at.clone())?.try_into()?;
        let entries = self.entries.lock().unwrap();
        let snapshot = entries.get(&handle.token).ok_or_else(expired)?;
        if snapshot.snapshot.at() != &at {
            return Err(expired());
        }
        Ok(snapshot.clone())
    }

    pub(crate) fn read(&self, handle: wire::SavedSnapshot) -> harmos_storage::Result<SnapshotRead<'_>> {
        let snapshot = self.get(&handle)?;
        Ok(SnapshotRead {
            owner: self,
            handle,
            snapshot,
            retained: false,
        })
    }
    pub(crate) fn release(&self, handle: &wire::SavedSnapshot) -> harmos_storage::Result<()> {
        let at: StorageCursor = harmos_storage::query::protobuf::required(handle.at.clone())?.try_into()?;
        let mut entries = self.entries.lock().unwrap();
        if entries.get(&handle.token).is_some_and(|snapshot| snapshot.snapshot.at() != &at) {
            return Err(expired());
        }
        entries.remove(&handle.token);
        Ok(())
    }
}

/* === Private Functions === */

fn expired() -> harmos_storage::Error {
    harmos_storage::Error::InvalidInput {
        field: "saved snapshot",
        reason: "snapshot has expired or belongs to another owner",
    }
}
