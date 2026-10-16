/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */
use crate::backend::storage_settings;
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use harmos::Error;
use harmos::storage::{
    FileMetadata, ObjectQuery, ObjectVersion, ObjectVersions, ReadHistory, ReadLimits,
    StorageCursor, StorageView, StreamWrite,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
struct Versioned {
    id: u64,
    value: u64,
}

#[harmos::message]
#[derive(Clone, PartialEq)]
struct OldVersioned {
    id: u64,
    value: u64,
}

#[harmos::message]
struct Conflicting {
    id: u64,
}

struct Fixture {
    runtime: Runtime<Bank>,
    storage: StorageManager<RetainedObjects>,
    primary: Arc<Data<RetainedObjects>>,
    replica: Option<Arc<Data<RetainedObjects>>>,
}

/* === Enums === */
#[harmos::catalog]
enum RetainedObjects {
    Counter(Counter),
    Versioned(Versioned),
    OldVersioned(OldVersioned),
}

#[harmos::catalog]
enum Disagreement {
    Versioned(Versioned),
    Conflicting(Conflicting),
}

/* === Traits === */

/* === Implementations === */
impl Identified for Versioned {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.versioned"), 2)
    }
}

impl Identified for OldVersioned {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.versioned"), 1)
    }
}

impl Identified for Conflicting {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("app.versioned"), 3)
    }
}

impl StoredObject for Versioned {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::Versions(NonZeroUsize::new(2).unwrap());
    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl StoredObject for OldVersioned {
    type Key = u64;
    const RETENTION: ObjectRetention = Versioned::RETENTION;
    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl StoredObject for Conflicting {
    type Key = u64;
    const RETENTION: ObjectRetention = ObjectRetention::Latest;
    fn key(&self) -> &Self::Key {
        &self.id
    }
}

impl Fixture {
    async fn new(replica: bool) -> Self {
        let primary = Arc::new(Data::<RetainedObjects>::with_catalog(history()));
        let replica = replica.then(|| Arc::new(Data::<RetainedObjects>::with_catalog(history())));
        if let Some(replica) = &replica {
            replica.control.block_open.store(true, Ordering::SeqCst);
        }
        let targets = replica
            .iter()
            .map(|replica| ReplicaTarget {
                id: Identifier::new("remote").unwrap(),
                backend: replica.clone(),
            })
            .collect();
        let storage = StorageManager::new(
            StorageBackends {
                primary: primary.clone(),
                state: Arc::new(Recovery::new(history())),
                replicas: targets,
            },
            StorageSettings {
                object_prune_entries: NonZeroUsize::MIN,
                flush_interval: Duration::from_secs(60),
                max_retries: 0,
                ..storage_settings()
            },
        )
        .unwrap();
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .runtime_with_storage(&storage)
            .await
            .unwrap();
        Self {
            runtime,
            storage,
            primary,
            replica,
        }
    }
    async fn finish(self) {
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn until(mut ready: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("retention makes bounded progress");
}

fn page_limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::MIN,
        bytes: limit(),
    }
}

fn revisions<T: StoredObject>() -> ObjectQuery<'static, T> {
    let mut query = ObjectQuery::new();
    query.versions = ObjectVersions::Revisions;
    query.options.page = Some(page_limits());
    query.options.decoded_bytes = Some(page_limits().bytes);
    query
}

async fn count_member<T: StoredObject>(
    storage: &StorageManager<RetainedObjects>,
    view: &StorageView,
) -> usize
where
    RetainedObjects: harmos::catalog::Contains<T>,
{
    let mut reader = storage.query_in(view, revisions::<T>()).unwrap();
    let mut count = 0;
    while let Some(page) = reader.next_page().await.unwrap() {
        count += page.len();
    }
    count
}

async fn count_retained(storage: &StorageManager<RetainedObjects>) -> usize {
    let view = storage
        .view_with_limits(storage_settings().query_execution)
        .await
        .unwrap();
    let counters = count_member::<Counter>(storage, &view).await;
    let current = count_member::<Versioned>(storage, &view).await;
    let historical = count_member::<OldVersioned>(storage, &view).await;
    counters + current + historical
}

async fn retained_source() -> (Fixture, StorageCursor) {
    let f = Fixture::new(false).await;
    let key = super::super::schema();
    f.storage
        .write_schema(
            &key,
            &harmos::schema::graph::Schema {
                root: 0,
                definitions: vec![vec![]],
            },
        )
        .await
        .unwrap();
    write(&f.storage, "retained-record").await;
    let item = Identifier::new("sample").unwrap();
    let stream = Identifier::new("signal").unwrap();
    f.storage
        .write_stream(&[StreamWrite {
            stream: &stream,
            sequence: 7,
            record: super::super::record(&key, &item, b"retained-row"),
        }])
        .await
        .unwrap();
    f.storage
        .write_file(
            FileMetadata {
                id: Identifier::new("bootstrap-file").unwrap(),
                scope: None,
                media_type: "application/octet-stream".into(),
                expected_length: None,
                expected_sha256: None,
            },
            b"verified retained bytes",
        )
        .await
        .unwrap();
    for value in 0..3 {
        f.storage
            .write_object(&Counter { id: 1, value })
            .await
            .unwrap();
    }
    let saved = f.storage.flush().await.unwrap();
    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
    f.storage.close().await.unwrap();
    f.primary
        .control
        .allow_bootstrap
        .store(true, Ordering::SeqCst);
    (f, saved)
}

async fn import_replica(
    f: &Fixture,
    replica: Arc<Data<RetainedObjects>>,
) -> StorageManager<RetainedObjects> {
    replica
        .control
        .allow_bootstrap
        .store(true, Ordering::SeqCst);
    let storage = StorageManager::new(
        StorageBackends {
            primary: f.primary.clone(),
            state: Arc::new(Recovery::new(history())),
            replicas: vec![ReplicaTarget {
                id: Identifier::new("new").unwrap(),
                backend: replica,
            }],
        },
        StorageSettings {
            max_retries: 0,
            ..storage_settings()
        },
    )
    .unwrap();
    storage.start(&f.runtime).await.unwrap();
    storage
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("pruning_preserves_pinned", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(false).await;
                    let first = f
                        .storage
                        .write_object(&Counter { id: 1, value: 10 })
                        .await
                        .unwrap();
                    f.storage.flush_until(&first.position).await.unwrap();
                    let pinned = f
                        .storage
                        .view_with_limits(storage_settings().query_execution)
                        .await
                        .unwrap();
                    let second = f
                        .storage
                        .write_object(&Counter { id: 1, value: 20 })
                        .await
                        .unwrap();
                    f.storage.flush_until(&second.position).await.unwrap();
                    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
                    assert_eq!(f.storage.cursor_saved(), second.position);
                    assert!(
                        f.storage
                            .read_object::<Counter>(
                                &1,
                                ObjectVersion::Exact(first.version.clone()),
                                limit()
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    let old = f
                        .storage
                        .read_object_in::<Counter>(
                            &pinned,
                            &1,
                            ObjectVersion::Exact(first.version),
                            limit(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(old.position, first.position);
                    let mut reader = f.storage.query_in(&pinned, revisions::<Counter>()).unwrap();
                    assert_eq!(reader.next_page().await.unwrap().unwrap().len(), 1);
                    assert!(reader.next_page().await.unwrap().is_none());
                    drop(reader);
                    drop(old);
                    until(|| f.primary.control.releases_completed.load(Ordering::SeqCst) >= 1)
                        .await;
                    let latest = f
                        .storage
                        .read_object::<Counter>(&1, ObjectVersion::Latest, limit())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(latest.value.value, 20);
                    drop(pinned);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("codecs_keep_distinct", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(false).await;
                    let first = f
                        .storage
                        .write_object(&OldVersioned { id: 1, value: 10 })
                        .await
                        .unwrap();
                    let second = f
                        .storage
                        .write_object(&OldVersioned { id: 1, value: 20 })
                        .await
                        .unwrap();
                    f.storage
                        .write_object(&Versioned { id: 1, value: 30 })
                        .await
                        .unwrap();
                    f.storage
                        .write_object(&Versioned { id: 2, value: 40 })
                        .await
                        .unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    until(|| f.primary.objects.lock().unwrap().len() == 3).await;
                    assert!(
                        f.storage
                            .read_object::<OldVersioned>(
                                &1,
                                ObjectVersion::Exact(first.version),
                                limit()
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    let retained = f
                        .storage
                        .read_object::<OldVersioned>(
                            &1,
                            ObjectVersion::Exact(second.version),
                            limit(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(retained.value.value, 20);
                    assert!(
                        f.storage
                            .read_object::<OldVersioned>(&1, ObjectVersion::Latest, limit())
                            .await
                            .is_err()
                    );
                    assert_eq!(count_retained(&f.storage).await, 3);
                    assert_eq!(f.storage.cursor_saved(), saved);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("lagging_replica_protects", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(true).await;
                    let replica = f.replica.as_ref().unwrap();
                    replica.control.open_entered.notified().await;
                    for value in 0..8 {
                        f.storage
                            .write_object(&Counter { id: 1, value })
                            .await
                            .unwrap();
                    }
                    let record = write(&f.storage, "preserved-record").await;
                    let saved = f.storage.flush().await.unwrap();
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 8);
                    assert!(f.primary.control.prune_calls.lock().unwrap().is_empty());
                    replica.control.block_open.store(false, Ordering::SeqCst);
                    replica.control.open_release.notify_one();
                    f.storage
                        .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    until(|| {
                        f.primary.objects.lock().unwrap().len() == 1
                            && replica.objects.lock().unwrap().len() == 1
                    })
                    .await;
                    assert_eq!(
                        f.primary.records.lock().unwrap().get(&record.sequence),
                        replica.records.lock().unwrap().get(&record.sequence)
                    );
                    for backend in [&f.primary, replica] {
                        assert!(
                            backend
                                .control
                                .prune_removed
                                .lock()
                                .unwrap()
                                .iter()
                                .all(|&removed| removed <= 1)
                        );
                        assert!(
                            backend
                                .control
                                .prune_calls
                                .lock()
                                .unwrap()
                                .iter()
                                .all(|&(through, _)| through <= saved.sequence)
                        );
                        assert_eq!(backend.objects.lock().unwrap().len(), 1);
                    }
                    assert_eq!(f.storage.cursor_saved(), saved);
                    assert_eq!(f.storage.replicas()[0].saved.as_ref(), Some(&saved));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("save_never_authorize", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(false).await;
                    for value in 0..2 {
                        f.storage
                            .write_object(&Counter { id: 1, value })
                            .await
                            .unwrap();
                    }
                    f.primary.control.flush_failures.store(1, Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 2);
                    assert!(f.primary.control.prune_calls.lock().unwrap().is_empty());
                    let saved = f.storage.flush().await.unwrap();
                    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
                    assert_eq!(f.storage.cursor_saved(), saved);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("pruning_retries_resurrecting", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(false).await;
                    f.primary.control.prune_failures.store(1, Ordering::SeqCst);
                    for value in 0..2 {
                        f.storage
                            .write_object(&Counter { id: 1, value })
                            .await
                            .unwrap();
                    }
                    let saved = f.storage.flush().await.unwrap();
                    until(|| f.storage.status().failure.is_some()).await;
                    assert_eq!(f.storage.cursor_saved(), saved);
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 1);
                    assert!(
                        f.storage
                            .write_object(&Counter { id: 1, value: 3 })
                            .await
                            .is_err()
                    );
                    f.storage.flush().await.unwrap();
                    until(|| {
                        f.primary.control.prune_calls.lock().unwrap().len() >= 2
                            && f.storage.status().failure.is_none()
                    })
                    .await;
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 1);
                    assert_eq!(f.storage.cursor_saved(), saved);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("close_waits_inflight", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(false).await;
    f.primary.control.block_prune.store(true, Ordering::SeqCst);
    f.storage
        .write_object(&Counter { id: 1, value: 1 })
        .await
        .unwrap();
    f.storage.flush().await.unwrap();
    f.primary.control.prune_entered.notified().await;
    let close = f.storage.close();
    tokio::pin!(close);
    tokio::select! {
        result = &mut close => panic!("close passed in-flight maintenance: {result:?}"),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }
    assert!(f.primary.control.open.load(Ordering::SeqCst));
    f.primary.control.block_prune.store(false, Ordering::SeqCst);
    f.primary.control.prune_release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), &mut close)
        .await
        .unwrap()
        .unwrap();
    assert!(!f.primary.control.open.load(Ordering::SeqCst));
    f.runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("queries_replay_reports", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(false).await;
                    for value in 0..3 {
                        f.storage
                            .write_object(&Counter { id: 1, value })
                            .await
                            .unwrap();
                    }
                    f.storage.flush().await.unwrap();
                    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
                    assert_eq!(count_retained(&f.storage).await, 1);
                    let mut complete = revisions::<Counter>();
                    complete.history = ReadHistory::Complete;
                    let mut replay = f.storage.replay(complete).await.unwrap();
                    assert!(matches!(replay.next_page().await, Err(Error::Gap { .. })));
                    assert!(f.storage.replay(revisions::<Counter>()).await.is_err());
                    assert!(f.storage.follow(revisions::<Counter>()).await.is_err());
                    drop(replay);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("replacement_must_reach", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(true).await;
                    let replica = f.replica.as_ref().unwrap();
                    replica.control.open_entered.notified().await;
                    replica.control.block_open.store(false, Ordering::SeqCst);
                    replica.control.open_release.notify_one();
                    let first = f
                        .storage
                        .write_object(&Counter { id: 1, value: 1 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    f.storage
                        .wait_replicas(&first.position, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();

                    replica.control.block_flush.store(true, Ordering::SeqCst);
                    let second = f
                        .storage
                        .write_object(&Counter { id: 1, value: 2 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    replica.control.flush_entered.notified().await;
                    let old = f
                        .storage
                        .read_object::<Counter>(&1, ObjectVersion::Exact(first.version), limit())
                        .await
                        .unwrap();
                    assert!(old.is_some());
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 2);
                    assert_eq!(
                        f.storage.replicas()[0].saved.as_ref(),
                        Some(&first.position)
                    );

                    replica.control.block_flush.store(false, Ordering::SeqCst);
                    replica.control.flush_release.notify_one();
                    f.storage
                        .wait_replicas(&second.position, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("history_reports_gap", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(false).await;
    for value in 0..3 {
        f.storage
            .write_object(&Counter { id: 1, value })
            .await
            .unwrap();
    }
    let saved = f.storage.flush().await.unwrap();
    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
    f.storage.close().await.unwrap();

    let replica = Arc::new(Data::<RetainedObjects>::with_catalog(history()));
    let reopened = StorageManager::new(
        StorageBackends {
            primary: f.primary.clone(),
            state: Arc::new(Recovery::new(history())),
            replicas: vec![ReplicaTarget {
                id: Identifier::new("new").unwrap(),
                backend: replica.clone(),
            }],
        },
        StorageSettings {
            max_retries: 0,
            ..storage_settings()
        },
    )
    .unwrap();
    reopened.start(&f.runtime).await.unwrap();
    let error = reopened
        .wait_replicas(&saved, &[Identifier::new("new").unwrap()])
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::Persistence { source } if matches!(source.as_ref(), Error::Gap { .. }))
    );
    assert_eq!(replica.saved.load(Ordering::SeqCst), 0);
    assert_eq!(reopened.cursor_saved(), saved);
    let latest = reopened
        .read_object::<Counter>(&1, ObjectVersion::Latest, limit())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.value.value, 2);
    reopened.close().await.unwrap();
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("failure_preserves_confirmed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(true).await;
                    let replica = f.replica.as_ref().unwrap();
                    replica.control.open_entered.notified().await;
                    replica.control.prune_failures.store(1, Ordering::SeqCst);
                    for value in 0..3 {
                        f.storage
                            .write_object(&Counter { id: 1, value })
                            .await
                            .unwrap();
                    }
                    let saved = f.storage.flush().await.unwrap();
                    replica.control.block_open.store(false, Ordering::SeqCst);
                    replica.control.open_release.notify_one();
                    until(|| f.storage.replicas()[0].failure.is_some()).await;
                    assert_eq!(f.storage.replicas()[0].saved.as_ref(), Some(&saved));
                    assert_eq!(f.storage.cursor_saved(), saved);
                    assert!(f.storage.status().failure.is_none());

                    f.storage
                        .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    until(|| {
                        replica.objects.lock().unwrap().len() == 1
                            && f.storage.replicas()[0].failure.is_none()
                    })
                    .await;
                    assert_eq!(f.storage.replicas()[0].saved.as_ref(), Some(&saved));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("bootstrap_publishes_collections", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (f, saved) = retained_source().await;
                    let target = Arc::new(Data::<RetainedObjects>::with_catalog(history()));
                    target
                        .control
                        .block_bootstrap_finish
                        .store(true, Ordering::SeqCst);
                    let storage = import_replica(&f, target.clone()).await;
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        target.control.bootstrap_finish_entered.notified(),
                    )
                    .await
                    .unwrap();
                    assert_eq!(target.saved.load(Ordering::SeqCst), 0);
                    assert!(target.schemas.lock().unwrap().is_empty());
                    assert!(target.records.lock().unwrap().is_empty());
                    assert!(target.rows.lock().unwrap().is_empty());
                    assert!(target.files.lock().unwrap().is_empty());
                    assert!(target.objects.lock().unwrap().is_empty());
                    assert_eq!(storage.replicas()[0].saved.as_ref().unwrap().sequence, 0);
                    target
                        .control
                        .block_bootstrap_finish
                        .store(false, Ordering::SeqCst);
                    target.control.bootstrap_finish_release.notify_one();
                    storage
                        .wait_replicas(&saved, &[Identifier::new("new").unwrap()])
                        .await
                        .unwrap();
                    assert_eq!(
                        *target.schemas.lock().unwrap(),
                        *f.primary.schemas.lock().unwrap()
                    );
                    assert_eq!(
                        *target.records.lock().unwrap(),
                        *f.primary.records.lock().unwrap()
                    );
                    assert_eq!(
                        *target.rows.lock().unwrap(),
                        *f.primary.rows.lock().unwrap()
                    );
                    assert_eq!(
                        *target.files.lock().unwrap(),
                        *f.primary.files.lock().unwrap()
                    );
                    assert_eq!(
                        *target.objects.lock().unwrap(),
                        *f.primary.objects.lock().unwrap()
                    );
                    assert_eq!(
                        *target.blobs.lock().unwrap(),
                        *f.primary.blobs.lock().unwrap()
                    );
                    assert_eq!(target.control.bootstrap_begins.load(Ordering::SeqCst), 1);
                    assert!(target.control.bootstrap_file_writes.load(Ordering::SeqCst) > 0);
                    assert_eq!(
                        target
                            .control
                            .bootstrap_file_finishes
                            .load(Ordering::SeqCst),
                        1
                    );
                    assert_eq!(
                        target.control.bootstrap_file_aborts.load(Ordering::SeqCst),
                        0
                    );
                    assert!(target.staged.lock().unwrap().is_empty());
                    storage.close().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("bootstrap_commit_reconciles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (f, saved) = retained_source().await;
                    let target = Arc::new(Data::<RetainedObjects>::with_catalog(history()));
                    target
                        .control
                        .uncertain_bootstrap_finish
                        .store(true, Ordering::SeqCst);
                    let storage = import_replica(&f, target.clone()).await;
                    until(|| storage.replicas()[0].failure.is_some()).await;
                    assert_eq!(target.saved.load(Ordering::SeqCst) as u64, saved.sequence);
                    assert_eq!(target.records.lock().unwrap().len(), 1);
                    assert_eq!(target.control.bootstrap_aborts.load(Ordering::SeqCst), 1);
                    storage
                        .wait_replicas(&saved, &[Identifier::new("new").unwrap()])
                        .await
                        .unwrap();
                    assert_eq!(target.control.bootstrap_begins.load(Ordering::SeqCst), 1);
                    assert_eq!(storage.replicas()[0].saved.as_ref(), Some(&saved));
                    assert_eq!(target.objects.lock().unwrap().len(), 1);
                    storage.close().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("abort_keeps_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (f, saved) = retained_source().await;
                    let target = Arc::new(Data::<RetainedObjects>::with_catalog(history()));
                    target
                        .control
                        .fail_bootstrap_begin
                        .store(true, Ordering::SeqCst);
                    target
                        .control
                        .fail_bootstrap_abort
                        .store(true, Ordering::SeqCst);
                    let storage = import_replica(&f, target.clone()).await;
                    until(|| storage.replicas()[0].failure.is_some()).await;
                    assert_eq!(target.saved.load(Ordering::SeqCst), 0);
                    assert!(f.primary.control.releases.load(Ordering::SeqCst) >= 2);
                    assert_eq!(target.control.bootstrap_begins.load(Ordering::SeqCst), 1);
                    target
                        .control
                        .fail_bootstrap_abort
                        .store(false, Ordering::SeqCst);
                    target
                        .control
                        .fail_bootstrap_begin
                        .store(false, Ordering::SeqCst);
                    storage
                        .wait_replicas(&saved, &[Identifier::new("new").unwrap()])
                        .await
                        .unwrap();
                    assert!(target.control.bootstrap_aborts.load(Ordering::SeqCst) >= 2);
                    storage.close().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("failure_retains_import", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (f, saved) = retained_source().await;
                    let target = Arc::new(Data::<RetainedObjects>::with_catalog(history()));
                    target.control.fail_finish.store(true, Ordering::SeqCst);
                    target.control.fail_abort.store(true, Ordering::SeqCst);
                    let storage = import_replica(&f, target.clone()).await;
                    until(|| storage.replicas()[0].failure.is_some()).await;
                    assert_eq!(target.saved.load(Ordering::SeqCst), 0);
                    assert_eq!(target.control.bootstrap_aborts.load(Ordering::SeqCst), 0);
                    assert!(target.control.bootstrap_file_aborts.load(Ordering::SeqCst) > 0);
                    assert!(!target.staged.lock().unwrap().is_empty());
                    assert!(f.primary.control.releases.load(Ordering::SeqCst) >= 2);
                    target.control.fail_finish.store(false, Ordering::SeqCst);
                    target.control.fail_abort.store(false, Ordering::SeqCst);
                    storage
                        .wait_replicas(&saved, &[Identifier::new("new").unwrap()])
                        .await
                        .unwrap();
                    assert!(target.control.bootstrap_file_aborts.load(Ordering::SeqCst) >= 2);
                    assert!(target.staged.lock().unwrap().is_empty());
                    assert_eq!(storage.replicas()[0].saved.as_ref(), Some(&saved));
                    storage.close().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("snapshot_release_rearms", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(false).await;
                    f.storage
                        .write_object(&Counter { id: 1, value: 10 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f
                        .storage
                        .view_with_limits(storage_settings().query_execution)
                        .await
                        .unwrap();
                    f.storage
                        .write_object(&Counter { id: 1, value: 20 })
                        .await
                        .unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
                    f.primary
                        .control
                        .block_release
                        .store(true, Ordering::SeqCst);
                    drop(view);
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        f.primary.control.release_entered.notified(),
                    )
                    .await
                    .unwrap();
                    let calls = f.primary.control.prune_calls.lock().unwrap().len();
                    assert_eq!(f.storage.cursor_saved(), saved);
                    assert_eq!(f.primary.control.prune_calls.lock().unwrap().len(), calls);
                    f.primary
                        .control
                        .block_release
                        .store(false, Ordering::SeqCst);
                    f.primary.control.release_release.notify_one();
                    until(|| f.primary.control.prune_calls.lock().unwrap().len() > calls).await;
                    assert_eq!(
                        f.primary
                            .control
                            .prune_calls
                            .lock()
                            .unwrap()
                            .last()
                            .unwrap()
                            .0,
                        saved.sequence
                    );
                    assert_eq!(f.storage.cursor_saved(), saved);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("source_release_rearms", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(true).await;
                    let replica = f.replica.as_ref().unwrap();
                    replica.control.open_entered.notified().await;
                    f.storage
                        .write_object(&Counter { id: 1, value: 10 })
                        .await
                        .unwrap();
                    let initial = f.storage.flush().await.unwrap();
                    replica.control.block_open.store(false, Ordering::SeqCst);
                    replica.control.open_release.notify_one();
                    // Finish Initial Release Synchronization Before Gating A Later Copy.
                    until(|| {
                        replica
                            .control
                            .prune_calls
                            .lock()
                            .unwrap()
                            .last()
                            .is_some_and(|call| call.0 == initial.sequence)
                    })
                    .await;
                    f.primary
                        .control
                        .block_release
                        .store(true, Ordering::SeqCst);
                    f.storage
                        .write_object(&Counter { id: 1, value: 20 })
                        .await
                        .unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        f.primary.control.release_entered.notified(),
                    )
                    .await
                    .unwrap();
                    until(|| {
                        f.primary
                            .control
                            .prune_calls
                            .lock()
                            .unwrap()
                            .last()
                            .is_some_and(|call| call.0 == saved.sequence)
                    })
                    .await;
                    let calls = f.primary.control.prune_calls.lock().unwrap().len();
                    assert_eq!(f.storage.replicas()[0].saved.as_ref(), Some(&saved));
                    f.primary
                        .control
                        .block_release
                        .store(false, Ordering::SeqCst);
                    f.primary.control.release_release.notify_one();
                    until(|| f.primary.control.prune_calls.lock().unwrap().len() > calls).await;
                    assert_eq!(
                        f.primary
                            .control
                            .prune_calls
                            .lock()
                            .unwrap()
                            .last()
                            .unwrap()
                            .0,
                        saved.sequence
                    );
                    assert_eq!(f.storage.cursor_saved(), saved);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("completion_preserves_replica", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(true).await;
                    let replica = f.replica.as_ref().unwrap();
                    replica.control.open_entered.notified().await;
                    replica.control.block_open.store(false, Ordering::SeqCst);
                    replica.control.open_release.notify_one();
                    // The Empty Initial Synchronization Has No Pruning Work To Gate.
                    until(|| f.primary.control.releases_completed.load(Ordering::SeqCst) > 0).await;
                    let released = f.primary.control.releases_completed.load(Ordering::SeqCst);
                    f.primary
                        .control
                        .block_release
                        .store(true, Ordering::SeqCst);
                    f.primary
                        .control
                        .block_prune_result
                        .store(true, Ordering::SeqCst);
                    f.storage
                        .write_object(&Counter { id: 1, value: 10 })
                        .await
                        .unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        f.primary.control.release_entered.notified(),
                    )
                    .await
                    .unwrap();
                    tokio::time::timeout(
                        Duration::from_secs(2),
                        f.primary.control.prune_result_entered.notified(),
                    )
                    .await
                    .unwrap();
                    let calls = f.primary.control.prune_calls.lock().unwrap().len();
                    f.primary
                        .control
                        .block_release
                        .store(false, Ordering::SeqCst);
                    f.primary.control.release_release.notify_one();
                    until(|| {
                        f.primary.control.releases_completed.load(Ordering::SeqCst) > released
                    })
                    .await;
                    // The Old Answer Was Already Calculated While The Source Pin Was Still Held.
                    f.primary
                        .control
                        .block_prune_result
                        .store(false, Ordering::SeqCst);
                    f.primary.control.prune_result_release.notify_one();
                    until(|| f.primary.control.prune_calls.lock().unwrap().len() > calls).await;
                    assert_eq!(
                        f.primary
                            .control
                            .prune_calls
                            .lock()
                            .unwrap()
                            .last()
                            .unwrap()
                            .0,
                        saved.sequence
                    );
                    assert_eq!(f.storage.replicas()[0].saved.as_ref(), Some(&saved));
                    assert_eq!(f.storage.cursor_saved(), saved);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("catalog_rejects_retention", || {
            assert!(Disagreement::validate().is_err());
            assert!(RetainedObjects::validate().is_ok());
            use harmos::catalog::Projection;
            let mut retentions = Vec::new();
            RetainedObjects::definitions()
                .try_for_each(|entry| {
                    retentions.push(entry.retention());
                    Ok::<(), std::convert::Infallible>(())
                })
                .unwrap();
            assert_eq!(retentions[0], ObjectRetention::Latest);
            assert_eq!(retentions[1].versions().get(), 2);
        });
    }
    {
        scenario_runtime::check("remote_receipts_hold", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::storage::ReplicaHolder;
                    let f = Fixture::new(false).await;
                    let mirror = Identifier::new("mirror").unwrap();
                    let holder = |generation| ReplicaHolder {
                        owner: "replica".into(),
                        generation,
                    };
                    let first = f
                        .storage
                        .write_object(&Counter { id: 1, value: 10 })
                        .await
                        .unwrap();
                    f.storage.flush_until(&first.position).await.unwrap();
                    f.storage
                        .acknowledge(&mirror, &holder(1), &first.position)
                        .unwrap();
                    let second = f
                        .storage
                        .write_object(&Counter { id: 1, value: 20 })
                        .await
                        .unwrap();
                    f.storage.flush_until(&second.position).await.unwrap();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    assert_eq!(
                        f.primary.objects.lock().unwrap().len(),
                        2,
                        "the receipt holds the older revision"
                    );

                    // Receipts Bind To One Owner And Never Regress Within A Generation
                    assert!(
                        f.storage
                            .acknowledge(
                                &mirror,
                                &ReplicaHolder {
                                    owner: "intruder".into(),
                                    generation: 9,
                                },
                                &second.position,
                            )
                            .is_err()
                    );
                    let mut earlier = first.position.clone();
                    earlier.sequence = 0;
                    assert!(
                        f.storage
                            .acknowledge(&mirror, &holder(1), &earlier)
                            .is_err()
                    );
                    let mut future = second.position.clone();
                    future.sequence += 10;
                    assert!(f.storage.acknowledge(&mirror, &holder(1), &future).is_err());

                    // A Newer Generation Rebinds And Fences The Older One
                    f.storage
                        .acknowledge(&mirror, &holder(2), &second.position)
                        .unwrap();
                    assert!(
                        f.storage
                            .acknowledge(&mirror, &holder(1), &second.position)
                            .is_err()
                    );
                    assert_eq!(f.storage.receipts()[0].holder, holder(2));
                    until(|| f.primary.objects.lock().unwrap().len() == 1).await;
                    f.storage.release_replica(&mirror, &holder(2)).unwrap();
                    assert!(f.storage.receipts().is_empty());
                    f.finish().await;
                });
        });
    }
}

/* === Tests === */
