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

use harmos_protocol::{storage as wire, v1};
use harmos_storage::{Snapshot, StorageCursor, StorageManager};
use ring::{digest, hmac};
use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

/* === Structs === */

pub struct SavedSnapshots {
    capacity: Arc<tokio::sync::Semaphore>,
    registry: Mutex<Registry>,
    issuance: tokio::sync::Mutex<()>,
    signing: Mutex<Option<hmac::Key>>,
    pub(super) queries: super::pages::Queries,
}

#[derive(Default)]
struct Registry {
    entries: BTreeMap<Vec<u8>, Arc<HeldSnapshot>>,
    order: VecDeque<Vec<u8>>,
}

pub(crate) struct SnapshotRead {
    handle: wire::SavedSnapshot,
    snapshot: Arc<HeldSnapshot>,
}

struct HeldSnapshot {
    snapshot: Arc<Snapshot>,
    cut: v1::ReadCut,
    explicit: bool,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

/* === Implementations === */

impl std::ops::Deref for SnapshotRead {
    type Target = Snapshot;
    fn deref(&self) -> &Self::Target {
        &self.snapshot.snapshot
    }
}
impl SnapshotRead {
    pub(crate) fn identity(&self) -> &[u8] {
        &self.handle.token
    }
    pub(crate) fn cut(&self) -> &v1::ReadCut {
        &self.snapshot.cut
    }
    pub(crate) fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.snapshot.clone()
    }
}

impl SavedSnapshots {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity: Arc::new(tokio::sync::Semaphore::new(capacity.get())),
            registry: Mutex::new(Registry::default()),
            issuance: tokio::sync::Mutex::new(()),
            signing: Mutex::new(None),
            queries: super::pages::Queries::new(capacity),
        }
    }
    pub(crate) fn reserve(&self) -> harmos_storage::Result<tokio::sync::OwnedSemaphorePermit> {
        let mut registry = self.registry.lock().unwrap();
        registry.entries.retain(|_, held| held.snapshot.is_available());
        let tokens = registry.entries.keys().cloned().collect::<std::collections::BTreeSet<_>>();
        registry.order.retain(|token| tokens.contains(token));
        while self.capacity.available_permits() == 0 {
            let index = registry.order.iter().position(|token| {
                registry.entries.get(token).is_some_and(|held| {
                    !held.explicit
                        && Arc::strong_count(held) == 1
                        && Arc::strong_count(&held.snapshot)
                            == registry.entries.values().filter(|other| Arc::ptr_eq(&other.snapshot, &held.snapshot)).count()
                })
            });
            let Some(index) = index else {
                let pinned = registry.entries.iter().filter(|(_, held)| held.explicit).map(|(token, _)| token.clone()).collect();
                if self.queries.evict_idle(Some(&pinned)) {
                    continue;
                }
                break;
            };
            let token = registry.order.remove(index).unwrap();
            registry.entries.remove(&token);
        }
        self.capacity.clone().try_acquire_owned().map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "saved snapshots" })
    }
    pub(crate) fn insert(
        &self, snapshot: Snapshot, permit: tokio::sync::OwnedSemaphorePermit,
    ) -> harmos_storage::Result<wire::SavedSnapshot> {
        let cut = super::transfers::cut(snapshot.at().clone());
        self.insert_cut(Arc::new(snapshot), cut, true, permit)
    }
    fn insert_cut(
        &self, snapshot: Arc<Snapshot>, cut: v1::ReadCut, explicit: bool, permit: tokio::sync::OwnedSemaphorePermit,
    ) -> harmos_storage::Result<wire::SavedSnapshot> {
        let mut token = vec![0; 32];
        getrandom::fill(&mut token).map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "snapshot entropy" })?;
        let at = Some(snapshot.at().clone().into());
        let mut registry = self.registry.lock().unwrap();
        registry.order.push_back(token.clone());
        registry.entries.insert(token.clone(), Arc::new(HeldSnapshot { snapshot, cut, explicit, _permit: permit }));
        Ok(wire::SavedSnapshot { token, at })
    }
    fn get(&self, handle: &wire::SavedSnapshot) -> harmos_storage::Result<Arc<HeldSnapshot>> {
        let at: StorageCursor = harmos_storage::query::protobuf::required(handle.at.clone())?.try_into()?;
        let registry = self.registry.lock().unwrap();
        let snapshot = registry.entries.get(&handle.token).ok_or_else(expired)?;
        if snapshot.snapshot.at() != &at {
            return Err(expired());
        }
        Ok(snapshot.clone())
    }
    pub(crate) fn read(&self, handle: wire::SavedSnapshot) -> harmos_storage::Result<SnapshotRead> {
        let snapshot = self.get(&handle)?;
        Ok(SnapshotRead { handle, snapshot })
    }
    pub(crate) fn release(&self, handle: &wire::SavedSnapshot) -> harmos_storage::Result<()> {
        let at: StorageCursor = harmos_storage::query::protobuf::required(handle.at.clone())?.try_into()?;
        let mut registry = self.registry.lock().unwrap();
        if registry.entries.get(&handle.token).is_some_and(|snapshot| snapshot.snapshot.at() != &at) {
            return Err(expired());
        }
        registry.entries.remove(&handle.token);
        registry.order.retain(|token| token != &handle.token);
        Ok(())
    }
    pub(crate) async fn create(
        &self, storage: &StorageManager, at: Option<v1::ReadCut>,
    ) -> crate::Result<(wire::SavedSnapshot, v1::ReadCut)> {
        if let Some(at) = at {
            let read = self.resolve(storage, Some(at)).await?;
            let permit = self.reserve()?;
            let cut = read.cut().clone();
            let handle = self.insert_cut(read.snapshot(), cut.clone(), true, permit)?;
            Ok((handle, cut))
        } else {
            let permit = self.reserve()?;
            let snapshot = storage.snapshot(&storage.get_saved_position()).await?;
            let cut = super::transfers::cut(snapshot.at().clone());
            let handle = self.insert(snapshot, permit)?;
            Ok((handle, cut))
        }
    }
    /// Resolve Only A Cut Previously Returned By This Owner, Or Select Its Current Durable Cut.
    pub(crate) async fn resolve(&self, storage: &StorageManager, at: Option<v1::ReadCut>) -> crate::Result<SnapshotRead> {
        let _issuance = self.issuance.lock().await;
        let explicit = at.is_some();
        let cut = at.unwrap_or_else(|| super::transfers::cut(storage.get_saved_position()));
        let found = {
            let registry = self.registry.lock().unwrap();
            registry.entries.iter().find(|(_, held)| held.cut == cut && held.snapshot.is_available()).map(|(token, held)| {
                wire::SavedSnapshot {
                    token: token.clone(),
                    at: Some(held.snapshot.at().clone().into()),
                }
            })
        };
        let handle = if let Some(handle) = found {
            handle
        } else {
            if explicit && !cut.revisions.is_empty() {
                return Err(expired().into());
            }
            let position = super::transfers::at(Some(cut.clone()))?.expect("one storage position");
            if explicit && !storage.check_receipt_position(&position)? {
                return Err(expired().into());
            }
            let permit = self.reserve()?;
            let snapshot = Arc::new(storage.snapshot(&position).await?);
            self.insert_cut(snapshot, cut, false, permit)?
        };
        let read = self.read(handle)?;
        Ok(read)
    }
    /// Retain A Cut Selected By The Storage/Provider Owner, Never An Unvalidated Caller Cut.
    pub(crate) async fn composite(&self, storage: &StorageManager, cut: v1::ReadCut) -> crate::Result<v1::ReadCut> {
        let _issuance = self.issuance.lock().await;
        if self.registry.lock().unwrap().entries.values().any(|held| held.cut == cut && held.snapshot.is_available()) {
            return Ok(cut);
        }
        let mut base = cut.clone();
        base.revisions.clear();
        let position = super::transfers::at(Some(base))?.expect("one storage history");
        let permit = self.reserve()?;
        let snapshot = Arc::new(storage.snapshot(&position).await?);
        self.insert_cut(snapshot, cut.clone(), false, permit)?;
        Ok(cut)
    }
    pub(crate) fn contains(&self, read: &SnapshotRead) -> bool {
        self.registry
            .lock()
            .unwrap()
            .entries
            .get(&read.handle.token)
            .is_some_and(|held| Arc::ptr_eq(held, &read.snapshot) && held.snapshot.is_available())
    }
    pub(crate) fn token(&self, identity: &[u8], plan: &[u8], sequence: u64) -> crate::Result<String> {
        let mut signing = self.signing.lock().unwrap();
        if signing.is_none() {
            let mut secret = [0; 32];
            getrandom::fill(&mut secret).map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "page token entropy" })?;
            *signing = Some(hmac::Key::new(hmac::HMAC_SHA256, &secret));
        }
        let mut bytes = identity.to_vec();
        bytes.extend_from_slice(&sequence.to_be_bytes());
        bytes.extend_from_slice(digest::digest(&digest::SHA256, plan).as_ref());
        bytes.extend_from_slice(hmac::sign(signing.as_ref().unwrap(), &bytes).as_ref());
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }
    pub(crate) fn decode_page(&self, token: &str, plan: &[u8]) -> crate::Result<(Vec<u8>, u64)> {
        let bytes = if token.len() == 208 {
            token
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| std::str::from_utf8(pair).ok().and_then(|pair| u8::from_str_radix(pair, 16).ok()).ok_or_else(expired))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            return Err(expired().into());
        };
        let signing = self.signing.lock().unwrap();
        let key = signing.as_ref().ok_or_else(expired)?;
        hmac::verify(key, &bytes[..72], &bytes[72..]).map_err(|_| expired())?;
        if &bytes[40..72] != digest::digest(&digest::SHA256, plan).as_ref() {
            return Err(expired().into());
        }
        Ok((bytes[..32].to_vec(), u64::from_be_bytes(bytes[32..40].try_into().unwrap())))
    }
}

/* === Private Functions === */

fn expired() -> harmos_storage::Error {
    harmos_storage::Error::InvalidInput {
        field: "storage continuation",
        reason: "issued cut or page token expired, changed, or belongs to another owner; restart the read",
    }
}
