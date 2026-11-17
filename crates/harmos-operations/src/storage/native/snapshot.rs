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

use super::file_access::Observation;
use super::pages::Queries;
use super::requests::SavedSnapshot;

use harmos_protocol::model;
use harmos_storage::{BackendSnapshot, Identifier, Snapshot, StorageCursor, StorageManager};
use ring::{digest, hmac};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    ops::Deref,
    str,
    sync::{Arc, Mutex},
};
use tokio::sync;

/* === Structs === */

pub struct SavedSnapshots {
    capacity: Arc<sync::Semaphore>,
    registry: Mutex<Registry>,
    pub(super) issuance: sync::Mutex<()>,
    signing: Mutex<Option<hmac::Key>>,
    pub(super) queries: Queries,
}

#[derive(Default)]
struct Registry {
    epoch: u64,
    entries: BTreeMap<Vec<u8>, Arc<HeldSnapshot>>,
    order: VecDeque<Vec<u8>>,
}

pub(crate) struct SnapshotRead {
    handle: SavedSnapshot,
    snapshot: Arc<HeldSnapshot>,
}

struct HeldSnapshot {
    epoch: u64,
    identity: Vec<u8>,
    snapshot: Arc<Snapshot>,
    cut: model::ReadCut,
    explicit: bool,
    observation: Option<Arc<Observation>>,
    _permit: sync::OwnedSemaphorePermit,
}

/* === Implementations === */

impl Deref for SnapshotRead {
    type Target = Snapshot;
    fn deref(&self) -> &Self::Target {
        &self.snapshot.snapshot
    }
}
impl SnapshotRead {
    pub(crate) fn identity(&self) -> &[u8] {
        &self.snapshot.identity
    }
    pub(crate) fn cut(&self) -> &model::ReadCut {
        &self.snapshot.cut
    }
    pub(super) fn observation(&self) -> Option<&Observation> {
        self.snapshot.observation.as_deref()
    }
    pub(crate) fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.snapshot.clone()
    }
}

impl SavedSnapshots {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            capacity: Arc::new(sync::Semaphore::new(capacity.get())),
            registry: Mutex::new(Registry::default()),
            issuance: sync::Mutex::new(()),
            signing: Mutex::new(None),
            queries: Queries::new(capacity),
        }
    }
    /// Call After a Deletion Commits, While Holding the Issuance Lock.
    pub(super) fn invalidate_implicit(&self) {
        let mut registry = self.registry.lock().unwrap();
        registry.epoch = registry.epoch.checked_add(1).expect("snapshot mutation epoch exhausted");
    }
    pub(crate) fn reserve(&self) -> harmos_storage::Result<sync::OwnedSemaphorePermit> {
        let mut registry = self.registry.lock().unwrap();
        registry.entries.retain(|_, held| held.snapshot.is_available());
        let tokens = registry.entries.keys().cloned().collect::<BTreeSet<_>>();
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
                let pinned = registry.entries.values().filter(|held| held.explicit).map(|held| held.identity.clone()).collect();
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
    fn insert_cut(
        &self, snapshot: Arc<Snapshot>, cut: model::ReadCut, explicit: bool, permit: sync::OwnedSemaphorePermit,
        observation: Option<Arc<Observation>>, identity: Option<Vec<u8>>,
    ) -> harmos_storage::Result<SavedSnapshot> {
        let mut token = vec![0; 32];
        getrandom::fill(&mut token).map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "snapshot entropy" })?;
        let identity = identity.unwrap_or_else(|| token.clone());
        if explicit {
            token = identity.clone();
        }
        let at = Some(snapshot.at().clone());
        let mut registry = self.registry.lock().unwrap();
        let epoch = registry.epoch;
        registry.order.push_back(token.clone());
        registry.entries.insert(
            token.clone(),
            Arc::new(HeldSnapshot {
                epoch,
                identity,
                snapshot,
                cut,
                explicit,
                observation,
                _permit: permit,
            }),
        );
        Ok(SavedSnapshot { token, at })
    }
    fn get(&self, handle: &SavedSnapshot) -> harmos_storage::Result<Arc<HeldSnapshot>> {
        let at: StorageCursor = super::requests::required(handle.at.clone())?;
        let registry = self.registry.lock().unwrap();
        let snapshot = registry.entries.get(&handle.token).ok_or_else(expired)?;
        if snapshot.snapshot.at() != &at || !snapshot.snapshot.is_available() {
            return Err(expired());
        }
        Ok(snapshot.clone())
    }
    pub(crate) fn read(&self, handle: SavedSnapshot) -> harmos_storage::Result<SnapshotRead> {
        let snapshot = self.get(&handle)?;
        Ok(SnapshotRead { handle, snapshot })
    }
    pub(crate) async fn release(&self, storage: &StorageManager, handle: &SavedSnapshot) -> harmos_storage::Result<()> {
        let retained = BackendSnapshot {
            at: super::requests::required(handle.at.clone())?,
            token: handle.token.clone(),
        };
        storage.release_retained_snapshot(&retained).await?;
        let mut registry = self.registry.lock().unwrap();
        registry.entries.retain(|_, held| held.identity != handle.token);
        let tokens = registry.entries.keys().cloned().collect::<BTreeSet<_>>();
        registry.order.retain(|token| tokens.contains(token));
        Ok(())
    }
    pub(crate) async fn read_saved(&self, storage: &StorageManager, handle: SavedSnapshot) -> crate::Result<SnapshotRead> {
        if let Ok(read) = self.read(handle.clone()) {
            return Ok(read);
        }
        let at = super::requests::required(handle.at)?;
        let mut cut = super::transfers::cut(at);
        cut.revisions.push(model::ResourceRevision {
            name: "storage.snapshot".into(),
            revision: handle.token,
        });
        self.resolve(storage, Some(cut)).await
    }
    pub(crate) async fn create(
        &self, storage: &StorageManager, at: Option<model::ReadCut>, scope: Option<&Identifier>,
    ) -> crate::Result<(SavedSnapshot, model::ReadCut)> {
        let permit = if scope.is_none() { Some(self.reserve()?) } else { None };
        let (retained, mut cut, observation) = if let Some(at) = at {
            let read = self.resolve(storage, Some(at)).await?;
            if scope.is_some() && read.cut().revisions.iter().any(|revision| revision.name != "storage.snapshot") {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "retained scope",
                    reason: "select a native data cut without provider observations or global projection revisions",
                }
                .into());
            }
            let mut cut = read.cut().clone();
            cut.revisions.retain(|revision| revision.name != "storage.snapshot");
            (read.retain(scope).await?, cut, read.snapshot.observation.clone())
        } else {
            let snapshot = storage.snapshot(&storage.get_saved_position()).await?;
            let cut = super::transfers::cut(snapshot.at().clone());
            (snapshot.retain(scope).await?, cut, None)
        };
        cut.revisions.push(model::ResourceRevision {
            name: "storage.snapshot".into(),
            revision: retained.token.clone(),
        });
        if let Some(permit) = permit {
            let snapshot = Arc::new(storage.open_retained_snapshot(&retained).await?);
            self.insert_cut(snapshot, cut.clone(), true, permit, observation, Some(retained.token.clone()))?;
        }
        Ok((
            SavedSnapshot {
                token: retained.token,
                at: Some(retained.at),
            },
            cut,
        ))
    }
    /// Resolve Only a Cut Previously Returned by This Owner, or Select Its Current Durable Cut.
    pub(crate) async fn resolve(&self, storage: &StorageManager, at: Option<model::ReadCut>) -> crate::Result<SnapshotRead> {
        let _issuance = self.issuance.lock().await;
        let explicit = at.is_some();
        let cut = at.unwrap_or_else(|| super::transfers::cut(storage.get_saved_position()));
        let found = {
            let registry = self.registry.lock().unwrap();
            registry
                .entries
                .iter()
                .find(|(_, held)| {
                    let current = explicit || held.epoch == registry.epoch;
                    current && held.cut == cut && held.snapshot.is_available()
                })
                .map(|(token, held)| SavedSnapshot {
                    token: token.clone(),
                    at: Some(held.snapshot.at().clone()),
                })
        };
        let handle = if let Some(handle) = found {
            handle
        } else {
            let retained = cut.revisions.iter().find(|revision| revision.name == "storage.snapshot");
            if explicit && (cut.revisions.iter().any(|revision| revision.name != "storage.snapshot") || cut.revisions.len() > 1) {
                return Err(expired().into());
            }
            let mut base = cut.clone();
            base.revisions.clear();
            let position = super::transfers::at(Some(base))?.expect("one storage position");
            let permit = self.reserve()?;
            let (snapshot, identity) = if let Some(retained) = retained {
                let handle = BackendSnapshot {
                    at: position,
                    token: retained.revision.clone(),
                };
                (storage.open_retained_snapshot(&handle).await?, Some(handle.token))
            } else {
                if explicit && !storage.check_receipt_position(&position)? {
                    return Err(expired().into());
                }
                (storage.snapshot(&position).await?, None)
            };
            self.insert_cut(Arc::new(snapshot), cut, false, permit, None, identity)?
        };
        let read = self.read(handle)?;
        Ok(read)
    }
    /// Retain a Cut Selected by the Storage/Provider Owner, Never an Unvalidated Caller Cut.
    pub(crate) async fn composite(&self, storage: &StorageManager, cut: model::ReadCut) -> crate::Result<model::ReadCut> {
        let _issuance = self.issuance.lock().await;
        if self.registry.lock().unwrap().entries.values().any(|held| held.cut == cut && held.snapshot.is_available()) {
            return Ok(cut);
        }
        let mut base = cut.clone();
        base.revisions.clear();
        let position = super::transfers::at(Some(base))?.expect("one storage history");
        let observation = self
            .registry
            .lock()
            .unwrap()
            .entries
            .values()
            .find(|held| {
                held.observation.is_some()
                    && held.cut.positions == cut.positions
                    && held.cut.revisions.iter().all(|revision| cut.revisions.contains(revision))
                    && held.snapshot.is_available()
            })
            .and_then(|held| held.observation.clone());
        let permit = self.reserve()?;
        let retained = cut.revisions.iter().find(|revision| revision.name == "storage.snapshot");
        let snapshot = if let Some(retained) = retained {
            storage
                .open_retained_snapshot(&BackendSnapshot {
                    at: position,
                    token: retained.revision.clone(),
                })
                .await?
        } else {
            storage.snapshot(&position).await?
        };
        self.insert_cut(Arc::new(snapshot), cut.clone(), false, permit, observation, retained.map(|retained| retained.revision.clone()))?;
        Ok(cut)
    }
    /// Attach Only Observed Metadata; No Accepted Storage Bytes Are Copied.
    pub(super) async fn observe(
        &self, storage: &StorageManager, position: StorageCursor, resource: String, observation: Observation,
    ) -> crate::Result<SnapshotRead> {
        let _issuance = self.issuance.lock().await;
        let permit = self.reserve()?;
        let snapshot = Arc::new(storage.snapshot(&position).await?);
        let mut revision = vec![0; 16];
        getrandom::fill(&mut revision)
            .map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "observation revision entropy" })?;
        let mut cut = super::transfers::cut(position);
        cut.revisions.push(model::ResourceRevision { name: resource, revision });
        let handle = self.insert_cut(snapshot, cut, false, permit, Some(Arc::new(observation)), None)?;
        Ok(self.read(handle)?)
    }
    pub(crate) fn seal_cursor<T: Serialize>(&self, read: &SnapshotRead, plan: &[u8], cursor: &T) -> crate::Result<String> {
        let mut signing = self.signing.lock().unwrap();
        if signing.is_none() {
            let mut secret = [0; 32];
            getrandom::fill(&mut secret).map_err(|_| harmos_storage::Error::ResourceExhausted { resource: "page token entropy" })?;
            *signing = Some(hmac::Key::new(hmac::HMAC_SHA256, &secret));
        }
        let mut bytes = b"storage.list:v1\0".to_vec();
        bytes.extend_from_slice(read.identity());
        bytes.extend_from_slice(digest::digest(&digest::SHA256, plan).as_ref());
        bytes.extend(postcard::to_allocvec(cursor).map_err(|_| expired())?);
        bytes.extend_from_slice(hmac::sign(signing.as_ref().unwrap(), &bytes).as_ref());
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }
    pub(crate) fn open_cursor<T: DeserializeOwned>(
        &self, token: &str, plan: &[u8], maximum: usize, at: Option<&model::ReadCut>,
    ) -> crate::Result<(SnapshotRead, T)> {
        const PREFIX: &[u8] = b"storage.list:v1\0";
        if token.len() > maximum || !token.len().is_multiple_of(2) {
            return Err(expired().into());
        }
        let bytes = token
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let high = (pair[0] as char).to_digit(16).ok_or_else(expired)?;
                let low = (pair[1] as char).to_digit(16).ok_or_else(expired)?;
                Ok::<_, harmos_storage::Error>(((high << 4) | low) as u8)
            })
            .collect::<harmos_storage::Result<Vec<_>>>()?;
        if bytes.len() <= PREFIX.len() + 96 || !bytes.starts_with(PREFIX) {
            return Err(expired().into());
        }
        let (message, signature) = bytes.split_at(bytes.len() - 32);
        let signing = self.signing.lock().unwrap();
        let key = signing.as_ref().ok_or_else(expired)?;
        hmac::verify(key, message, signature).map_err(|_| expired())?;
        let identity = &message[PREFIX.len()..PREFIX.len() + 32];
        if &message[PREFIX.len() + 32..PREFIX.len() + 64] != digest::digest(&digest::SHA256, plan).as_ref() {
            return Err(expired().into());
        }
        let cursor = postcard::from_bytes(&message[PREFIX.len() + 64..]).map_err(|_| expired())?;
        let handle = {
            let registry = self.registry.lock().unwrap();
            let (token, held) =
                registry.entries.iter().find(|(_, held)| held.identity == identity && held.snapshot.is_available()).ok_or_else(expired)?;
            SavedSnapshot {
                token: token.clone(),
                at: Some(held.snapshot.at().clone()),
            }
        };
        let read = self.read(handle)?;
        if at.is_some_and(|at| at != read.cut()) {
            return Err(expired().into());
        }
        Ok((read, cursor))
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
                .map(|pair| str::from_utf8(pair).ok().and_then(|pair| u8::from_str_radix(pair, 16).ok()).ok_or_else(expired))
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

/* === Tests === */

#[cfg(test)]
mod tests {
    /* === Imports === */

    use std::num::{NonZeroU32, NonZeroUsize};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::sync;
    use tokio::task::JoinHandle;

    use super::super::watch_rows::{self, Event};
    use super::super::{
        DeleteFile, DeleteFileRequest, DeleteObject, DeleteObjectRequest, DeleteScope, DeleteScopeRequest, GetFileBytes,
        GetFileBytesRequest, Mutate, MutateRequest, object,
    };
    use super::SavedSnapshots;
    use crate::execution::operations::{ExecutionContext, NativeOperation, ResourceSet};
    use harmos_domain::Identified;
    use harmos_protocol::model;
    use harmos_storage::memory::{Backend, RecoveryBackend};
    use harmos_storage::query::{RecordSelection, Source};
    use harmos_storage::{
        CompoundOutcome, FileMetadata, Identifier, MutationId, MutationItem, ObjectCatalog, ObjectDefinition, ObjectRetention, ObjectValue,
        PayloadEncoding, RecordInput, SchemaKey, StagedFile, StorageBackend, StorageBackends, StorageFilesystemSettings, StorageFuture,
        StorageManager, StorageRequestSettings, StorageResponseSettings, StorageSettings, StoreHistory, StoredObject, WorkerOwner,
    };

    /* === Structs === */

    struct Task(sync::Mutex<Option<JoinHandle<harmos_storage::Result<()>>>>);

    #[derive(harmos_domain::Message, Clone, Debug, PartialEq)]
    struct Evidence {
        id: u64,
        scope: Identifier,
    }

    /* === Implementations === */

    impl WorkerOwner for Task {
        fn stop(&self) -> StorageFuture<'_, ()> {
            Box::pin(async move { self.0.lock().await.take().unwrap().await.unwrap() })
        }
    }

    impl Identified for Evidence {
        const ID: &'static str = "native.evidence";
    }

    impl StoredObject for Evidence {
        type Key = u64;
        const RETENTION: ObjectRetention = ObjectRetention::Latest;

        fn key(&self) -> &u64 {
            &self.id
        }

        fn scope(&self) -> Option<&Identifier> {
            Some(&self.scope)
        }
    }

    /* === Private Functions === */

    async fn start(primary: Arc<Backend>) -> StorageManager {
        let count = NonZeroUsize::new(64).unwrap();
        let storage = StorageManager::with_catalog(
            ObjectCatalog::new().entry(ObjectDefinition::of::<Evidence>()),
            StorageBackends {
                primary: primary.clone(),
                state: Arc::new(RecoveryBackend::new(primary.history.clone())),
            },
            StorageSettings {
                transfer: Default::default(),
                policy: Default::default(),
                filesystem: StorageFilesystemSettings {
                    poll_interval: Duration::from_millis(100),
                    max_entries: count,
                    event_capacity: count,
                },
                request: StorageRequestSettings {
                    max_file_productions: count,
                    max_mutation_receipts: count,
                    max_pending_queries: count,
                    max_active_queries: count,
                    max_pending_write_batches: count,
                    max_active_file_operations: count,
                    max_pending_write_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
                },
                response: StorageResponseSettings {
                    batch_rows: count,
                    batch_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
                },
            },
        )
        .unwrap();
        storage
            .start_with(Arc::new(()), |mut worker| {
                let task = tokio::spawn(async move {
                    worker.run().await?;
                    worker.cleanup().await
                });
                Ok(Box::new(Task(sync::Mutex::new(Some(task)))))
            })
            .await
            .unwrap();
        storage
    }

    fn primary() -> Arc<Backend> {
        Arc::new(Backend::with_catalog(
            ObjectCatalog::new().entry(ObjectDefinition::of::<Evidence>()),
            StoreHistory {
                store: Identifier::new("native-retention").unwrap(),
                epoch: Identifier::new("epoch").unwrap(),
            },
        ))
    }

    fn mutation(storage: &StorageManager, key: &str) -> MutationId {
        MutationId {
            window: storage.mutation_window().unwrap().id,
            key: Identifier::new(key).unwrap(),
        }
    }

    async fn object_fixture() -> (StorageManager, ExecutionContext, ObjectValue) {
        let storage = start(primary()).await;
        let catalog = ObjectCatalog::new().entry(ObjectDefinition::of::<Evidence>()).build().unwrap();
        let value = catalog
            .encode(
                &Evidence {
                    id: 1,
                    scope: Identifier::new("evidence").unwrap(),
                },
                NonZeroUsize::MAX,
            )
            .unwrap();
        storage.mutate_once(&mutation(&storage, "create"), vec![MutationItem::Create(value.clone())]).await.unwrap();
        let mut resources = ResourceSet::new();
        resources.insert(Arc::new(storage.clone())).unwrap();
        resources.insert(Arc::new(SavedSnapshots::new(NonZeroUsize::new(8).unwrap()))).unwrap();
        let context = ExecutionContext::new(resources, Default::default());
        (storage, context, value)
    }

    /* === Tests === */

    #[tokio::test]
    async fn implicit_read_observes_object_deletion() {
        // Open an Implicit Snapshot Before Deleting the Object.
        let (storage, context, value) = object_fixture().await;
        let before = object::get(&context, Some(value.schema.clone()), value.item.to_string(), None).await.unwrap();
        assert_eq!(before.0, Some(value.clone()));
        let request = DeleteObjectRequest {
            mutation: Some(mutation(&storage, "delete")),
            schema: value.schema.id.clone(),
            item: value.item.clone(),
            expected: None,
        };
        let response = DeleteObject::run(context.clone(), request, &()).await.unwrap();
        assert!(matches!(response.outcome, CompoundOutcome::Applied { .. }));

        // An Unqualified Read Must Observe the Committed Deletion.
        let after = object::get(&context, Some(value.schema), value.item.to_string(), None).await.unwrap();
        assert!(after.0.is_none());
        storage.close().await.unwrap();
    }

    #[tokio::test]
    async fn implicit_read_observes_scope_deletion() {
        // Open an Implicit Snapshot Before Deleting Its Scope.
        let (storage, context, value) = object_fixture().await;
        let before = object::get(&context, Some(value.schema.clone()), value.item.to_string(), None).await.unwrap();
        assert_eq!(before.0, Some(value.clone()));
        let request = DeleteScopeRequest {
            mutation: Some(mutation(&storage, "delete-scope")),
            scope: value.scope.clone().unwrap(),
        };
        let response = DeleteScope::run(context.clone(), request, &()).await.unwrap();
        assert!(matches!(response.outcome, CompoundOutcome::Applied { .. }));

        // An Unqualified Read Must Observe the Committed Scope Deletion.
        let after = object::get(&context, Some(value.schema), value.item.to_string(), None).await.unwrap();
        assert!(after.0.is_none());
        storage.close().await.unwrap();
    }

    #[tokio::test]
    async fn explicit_read_keeps_pre_deletion_image() {
        // Preserve a Durable Retained Cut Through Each Delete.
        for scope in [false, true] {
            let (storage, context, value) = object_fixture().await;
            let before = object::get(&context, Some(value.schema.clone()), value.item.to_string(), None).await.unwrap();
            assert_eq!(before.0, Some(value.clone()));
            let snapshots = context.resource::<SavedSnapshots>().unwrap();
            let (_, retained) = snapshots.create(&storage, Some(before.2.clone()), None).await.unwrap();
            let item = if scope {
                MutationItem::DeleteScope(value.scope.clone().unwrap())
            } else {
                MutationItem::Delete {
                    schema: value.schema.id.clone(),
                    item: value.item.clone(),
                    expected: None,
                }
            };
            let request = MutateRequest {
                mutation: Some(mutation(&storage, "delete")),
                items: vec![item],
            };
            let response = Mutate::run(context.clone(), request, &()).await.unwrap();
            assert!(matches!(response.outcome, CompoundOutcome::Applied { .. }));

            let held = object::get(&context, Some(value.schema.clone()), value.item.to_string(), Some(before.2)).await.unwrap();
            assert_eq!(held.0, Some(value.clone()));

            // Explicit Reads Keep Their Original Image Even After an Implicit Refresh.
            let after = object::get(&context, Some(value.schema.clone()), value.item.to_string(), None).await.unwrap();
            assert!(after.0.is_none());
            let held = object::get(&context, Some(value.schema.clone()), value.item.to_string(), Some(retained)).await.unwrap();
            assert_eq!(held.0, Some(value));
            storage.close().await.unwrap();
        }
    }

    #[tokio::test]
    async fn implicit_read_observes_compound_deletion() {
        // Put Two Objects and Open an Implicit Snapshot Before Deleting Both.
        let (storage, context, value) = object_fixture().await;
        let catalog = ObjectCatalog::new().entry(ObjectDefinition::of::<Evidence>()).build().unwrap();
        let second = catalog
            .encode(
                &Evidence {
                    id: 2,
                    scope: value.scope.clone().unwrap(),
                },
                NonZeroUsize::MAX,
            )
            .unwrap();
        storage.mutate_once(&mutation(&storage, "create-second"), vec![MutationItem::Create(second.clone())]).await.unwrap();
        let before = object::get(&context, Some(value.schema.clone()), value.item.to_string(), None).await.unwrap();
        assert_eq!(before.0, Some(value.clone()));
        let items = [&value, &second]
            .map(|value| MutationItem::Delete {
                schema: value.schema.id.clone(),
                item: value.item.clone(),
                expected: None,
            })
            .to_vec();
        let request = MutateRequest {
            mutation: Some(mutation(&storage, "delete-both")),
            items,
        };
        let response = Mutate::run(context.clone(), request, &()).await.unwrap();
        assert!(matches!(response.outcome, CompoundOutcome::Applied { .. }));

        // Each Unqualified Read Must Observe the Entire Committed Mutation.
        for value in [value, second] {
            let after = object::get(&context, Some(value.schema), value.item.to_string(), None).await.unwrap();
            assert!(after.0.is_none());
        }
        storage.close().await.unwrap();
    }

    #[tokio::test]
    async fn implicit_read_observes_file_deletion() {
        // Publish a File and Open Its Implicit Snapshot.
        let primary = primary();
        let storage = start(primary.clone()).await;
        let mut resources = ResourceSet::new();
        resources.insert(Arc::new(storage.clone())).unwrap();
        resources.insert(Arc::new(SavedSnapshots::new(NonZeroUsize::new(8).unwrap()))).unwrap();
        let context = ExecutionContext::new(resources, Default::default());
        let mut writer = storage
            .produce_file(FileMetadata {
                id: Identifier::new("file").unwrap(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            })
            .await
            .unwrap();
        writer.write_all(b"evidence".to_vec()).await.unwrap();
        let file = writer.finish().await.unwrap();
        storage.flush().await.unwrap();
        let request = GetFileBytesRequest {
            file: Some(file.clone()),
            offset: 0,
            max_bytes: 8,
            at: None,
        };
        let before = GetFileBytes::run(context.clone(), request.clone(), &()).await.unwrap();
        assert_eq!(before.data, b"evidence");
        let snapshots = context.resource::<SavedSnapshots>().unwrap();
        let (_, cut) = snapshots.create(&storage, before.resolved_at, None).await.unwrap();
        let position = storage.get_saved_position();

        // Delete Through the Native Operation Used by Clients.
        let deletion = DeleteFileRequest {
            mutation: Some(mutation(&storage, "delete-file")),
            file: Some(StagedFile {
                history: file.position.history,
                id: file.metadata.id,
            }),
        };
        DeleteFile::run(context.clone(), deletion, &()).await.unwrap();
        assert_eq!(storage.get_saved_position(), position);
        assert!(GetFileBytes::run(context.clone(), request.clone(), &()).await.is_err());

        // The Previously Issued Explicit Cut Still Reads Its Retained Bytes.
        let retained = GetFileBytesRequest { at: Some(cut), ..request };
        let held = GetFileBytes::run(context, retained, &()).await.unwrap();
        assert_eq!(held.data, b"evidence");
        storage.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_retained_handles_reopen_after_owner_and_storage_restart() {
        // Persist Both a Scoped Owner and an Independent Snapshot Handle.
        let primary = primary();
        let storage = start(primary.clone()).await;
        let snapshots = SavedSnapshots::new(NonZeroUsize::new(8).unwrap());
        let scope = Identifier::new("execution").unwrap();
        let mut writer = storage
            .produce_file(FileMetadata {
                id: Identifier::new("file").unwrap(),
                scope: Some(scope.clone()),
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            })
            .await
            .unwrap();
        writer.write_all(b"evidence".to_vec()).await.unwrap();
        writer.finish().await.unwrap();
        storage.flush().await.unwrap();
        let (owner, cut) = snapshots.create(&storage, None, Some(&scope)).await.unwrap();
        let (independent, independent_cut) = snapshots.create(&storage, Some(cut.clone()), None).await.unwrap();
        drop(snapshots);
        storage.close().await.unwrap();
        drop(storage);

        // Recreate the Entire Native Registry and Both Storage Sessions.
        let storage = start(primary.clone()).await;
        let snapshots = SavedSnapshots::new(NonZeroUsize::new(8).unwrap());
        assert_eq!(snapshots.resolve(&storage, Some(cut.clone())).await.unwrap().cut(), &cut);
        assert_eq!(snapshots.read_saved(&storage, independent.clone()).await.unwrap().cut(), &independent_cut);
        let reader = snapshots.read_saved(&storage, owner.clone()).await.unwrap();
        let copy = Arc::new(Backend::new(primary.history.clone()));
        let source = storage.snapshot(&storage.get_saved_position()).await.unwrap();
        source.copy_to(copy.clone()).await.unwrap();
        drop(source);
        storage.mutate_once(&mutation(&storage, "delete-scope"), vec![MutationItem::DeleteScope(scope.clone())]).await.unwrap();
        assert!(reader.is_available());
        assert!(snapshots.create(&storage, Some(independent_cut.clone()), Some(&scope)).await.is_err());
        snapshots.release(&storage, &owner).await.unwrap();
        assert!(snapshots.resolve(&storage, Some(cut.clone())).await.is_err());
        assert!(snapshots.read_saved(&storage, independent.clone()).await.unwrap().is_available());
        snapshots.release(&storage, &independent).await.unwrap();
        snapshots.release(&storage, &independent).await.unwrap();
        drop(reader);
        drop(snapshots);
        primary.snapshots_released().await;
        assert_eq!(primary.retained_file_bytes(), 0);
        storage.close().await.unwrap();

        // A Backend Copy Keeps Its Own Durable Snapshot Ownership.
        let restored = start(copy.clone()).await;
        let snapshots = SavedSnapshots::new(NonZeroUsize::new(8).unwrap());
        assert_eq!(snapshots.resolve(&restored, Some(cut)).await.unwrap().at(), owner.at.as_ref().unwrap());
        drop(snapshots);
        restored.close().await.unwrap();
        copy.dispose().await.unwrap();
        assert_eq!(copy.retained_file_bytes(), 0);
    }

    #[tokio::test]
    async fn native_watch_never_issues_partial_compound_cuts_and_resyncs_scope_deletion() {
        // Register the Native Watch Before Admitting a Compound Mutation.
        let primary = primary();
        let storage = start(primary.clone()).await;
        let snapshots = Arc::new(SavedSnapshots::new(NonZeroUsize::new(8).unwrap()));
        let mut resources = ResourceSet::new();
        resources.insert(Arc::new(storage.clone())).unwrap();
        resources.insert(snapshots.clone()).unwrap();
        let context = ExecutionContext::new(resources, Default::default());
        let schema = SchemaKey {
            id: Identifier::new("rows").unwrap(),
            version: NonZeroU32::new(1).unwrap(),
            payload_encoding: PayloadEncoding::PostcardV1,
        };
        let scope = Identifier::new("watched-scope").unwrap();
        let record = RecordInput {
            schema: schema.clone(),
            scope: Some(scope.clone()),
            item: Identifier::new("selected").unwrap(),
            source: None,
            source_time: None,
            payload: vec![0; 40_000],
        };
        let selection = Source::Records(RecordSelection {
            schema,
            scope: Some(scope.clone()),
            item: Some(record.item.clone()),
            source_start: None,
            source_end: None,
        });
        let frames = Arc::new(Mutex::new(Vec::new()));
        let captured = frames.clone();
        let final_frame = watch_rows::run(&context, Some(selection.clone()), None, |frame| {
            let storage = storage.clone();
            let scope = scope.clone();
            let record = record.clone();
            let captured = captured.clone();
            async move {
                match &frame.event {
                    Event::Boundary => {
                        let mut unmatched = record.clone();
                        unmatched.item = Identifier::new("unmatched").unwrap();
                        storage
                            .mutate_once(
                                &mutation(&storage, "compound"),
                                vec![
                                    MutationItem::Append(record.clone()),
                                    MutationItem::Append(record.clone()),
                                    MutationItem::Append(record),
                                    MutationItem::Append(unmatched),
                                ],
                            )
                            .await
                            .unwrap();
                    }
                    Event::Progress => {
                        storage.mutate_once(&mutation(&storage, "delete"), vec![MutationItem::DeleteScope(scope)]).await.unwrap();
                    }
                    _ => {}
                }
                captured.lock().unwrap().push(frame);
                Ok(())
            }
        })
        .await
        .unwrap();
        assert!(matches!(final_frame.event, Event::Resync(_)));
        {
            let frames = frames.lock().unwrap();
            assert_eq!(frames.len(), 6);
            for frame in &frames[1..4] {
                assert!(matches!(frame.event, Event::Record(_)));
                assert_eq!(frame.at.positions[0].sequence, 0);
            }
            assert_eq!(frames[4].at.positions[0].sequence, 4);
        }
        let mut expired = final_frame.at.clone();
        expired.positions[0].sequence = 1;
        expired.revisions.push(model::ResourceRevision {
            name: "storage.snapshot".into(),
            revision: b"released".to_vec(),
        });
        let terminal = watch_rows::run(&context, Some(selection), Some(expired), |frame| async move {
            assert!(matches!(frame.event, Event::Resync(_)));
            Ok(())
        })
        .await
        .unwrap();
        assert!(matches!(terminal.event, Event::Resync(_)));
        assert_eq!(terminal.at.positions[0].sequence, storage.get_saved_position().sequence);
        drop(context);
        drop(snapshots);
        primary.snapshots_released().await;
        storage.close().await.unwrap();
    }
}
