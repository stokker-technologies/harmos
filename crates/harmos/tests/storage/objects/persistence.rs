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

#[path = "keys.rs"]
mod keys;

#[path = "versions.rs"]
mod versions;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use crate::{record, schema};
use harmos::schema::Described;
use harmos::storage::{
    FileMetadata, ObjectQuery, ObjectVersion, ObjectVersions, ReadHistory, ReadLimits,
    StorageBackend, StreamWrite,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Fixture {
    runtime: Runtime<Bank>,
    storage: StorageManager<Objects>,
    primary: Arc<Data<Objects>>,
    replica: Arc<Data<Objects>>,
    execution: harmos::storage::QueryLimits,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Fixture {
    async fn new(config: StorageSettings) -> Self {
        Self::with_replica(config, false).await
    }
    async fn with_replica(mut config: StorageSettings, blocked: bool) -> Self {
        let primary = Arc::new(Data::<Objects>::with_catalog(history()));
        let replica = Arc::new(Data::<Objects>::with_catalog(history()));
        replica.control.block_open.store(blocked, Ordering::SeqCst);
        config.flush_interval = Duration::from_secs(60);
        config.max_retries = 0;
        let replicas = vec![ReplicaTarget {
            id: Identifier::new("remote").unwrap(),
            backend: replica.clone(),
        }];
        let execution = config.query_execution;
        let storage = StorageManager::new(
            StorageBackends {
                primary: primary.clone(),
                recovery: Arc::new(Recovery::new(history())),
                replicas,
            },
            config,
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
            execution,
        }
    }
    async fn finish(self) {
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

fn key() -> String {
    "document-1".to_owned()
}

fn revisions<'a>() -> ObjectQuery<'a, Document> {
    let mut query = ObjectQuery::new();
    query.versions = ObjectVersions::Revisions;
    query.history = ReadHistory::Complete;
    query.options.page = Some(limits());
    query.options.decoded_bytes = Some(limits().bytes);
    query
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("updates_batch_losing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let first = f.storage.write_object(&document("running")).await.unwrap();
                    let second = f.storage.write_object(&document("finished")).await.unwrap();
                    assert_eq!(second.position.sequence, first.position.sequence + 2);
                    assert!(f.primary.objects.lock().unwrap().is_empty());
                    assert!(
                        f.storage
                            .read_object::<Document>(&key(), ObjectVersion::Latest, limits().bytes)
                            .await
                            .unwrap()
                            .is_none()
                    );
                    let saved = f.storage.flush().await.unwrap();
                    assert_eq!(saved, second.position);
                    assert_eq!(
                        f.primary.control.write_sizes.lock().unwrap().as_slice(),
                        &[4]
                    );
                    let current = f
                        .storage
                        .read_object::<Document>(&key(), ObjectVersion::Latest, limits().bytes)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(current.value.status, "finished");
                    assert_eq!(current.position, second.position);
                    let old = f
                        .storage
                        .read_object::<Document>(
                            &key(),
                            ObjectVersion::Exact(first.version.clone()),
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(old.value.status, "running");
                    assert_eq!(old.position, first.position);
                    assert!(
                        f.primary.object_schemas.lock().unwrap()[&("app.document".to_owned(), 2)]
                            .iter()
                            .any(|field| field.name == "status")
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("views_preserve_values", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let first = f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let mut history = f.storage.query_in(&view, revisions()).unwrap();
                    f.storage.write_object(&document("finished")).await.unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    f.storage
                        .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    // The physical fixture discards its current table; the pinned snapshot owns data.
                    f.primary.objects.lock().unwrap().clear();
                    let object = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &key(),
                            ObjectVersion::Latest,
                            limits().bytes,
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(object.position, first.position);
                    assert_eq!(object.value.status, "running");
                    assert_eq!(history.next_page().await.unwrap().unwrap().len(), 1);
                    assert!(history.next_page().await.unwrap().is_none());
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("latest_survives_pruning", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let first = f.storage.write_object(&document("running")).await.unwrap();
                    let second = f.storage.write_object(&document("finished")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    f.primary
                        .objects
                        .lock()
                        .unwrap()
                        .remove(&first.position.sequence);
                    f.primary
                        .control
                        .lost_objects_through
                        .store(first.position.sequence as usize, Ordering::SeqCst);
                    let current = f
                        .storage
                        .read_object::<Document>(&key(), ObjectVersion::Latest, limits().bytes)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(current.position, second.position);
                    assert!(
                        f.storage
                            .read_object::<Document>(
                                &key(),
                                ObjectVersion::Exact(first.version.clone()),
                                limits().bytes,
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("codec_never_silently", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let old = LegacyDocument {
                        id: "document-1".into(),
                        status: "old".into(),
                    };
                    let first = f.storage.write_object(&old).await.unwrap();
                    f.storage.write_object(&document("new")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let id = key();
                    assert!(
                        f.storage
                            .read_object::<LegacyDocument>(
                                &id,
                                ObjectVersion::Latest,
                                limits().bytes
                            )
                            .await
                            .is_err()
                    );
                    assert_eq!(
                        f.storage
                            .read_object::<LegacyDocument>(
                                &id,
                                ObjectVersion::Exact(first.version.clone()),
                                limits().bytes
                            )
                            .await
                            .unwrap()
                            .unwrap()
                            .value,
                        old
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("mixed_collections_replicate", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings {
                        flush_interval: Duration::from_secs(60),
                        ..StorageSettings::default()
                    })
                    .await;
                    f.storage
                        .write_schema(
                            &schema(),
                            &harmos::schema::graph::Schema {
                                root: 0,
                                definitions: vec![vec![]],
                            },
                        )
                        .await
                        .unwrap();
                    f.storage.write_object(&document("running")).await.unwrap();
                    write(&f.storage, "record").await;
                    f.storage.write_object(&document("finished")).await.unwrap();
                    f.storage
                        .write_stream(&[StreamWrite {
                            stream: &Identifier::new("stream").unwrap(),
                            sequence: 1,
                            record: record(&schema(), &Identifier::new("row").unwrap(), b"row"),
                        }])
                        .await
                        .unwrap();
                    f.storage
                        .write_file(
                            FileMetadata {
                                id: Identifier::new("file").unwrap(),
                                scope: None,
                                media_type: "application/octet-stream".into(),
                                expected_length: Some(3),
                                expected_sha256: None,
                            },
                            b"abc",
                        )
                        .await
                        .unwrap();
                    // Sealing the last file must not publish it ahead of the earlier queued
                    // collections or force their persistence as a side effect of finish.
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    assert_eq!(f.primary.blobs.lock().unwrap().len(), 1);
                    assert!(f.primary.files.lock().unwrap().is_empty());
                    assert!(f.primary.control.writes.lock().unwrap().is_empty());
                    let saved = f.storage.flush().await.unwrap();
                    assert_eq!(
                        *f.primary.control.writes.lock().unwrap(),
                        [1, 2, 3, 4, 5, 6, 7, 8]
                    );
                    f.storage
                        .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    assert_eq!(f.replica.objects.lock().unwrap().len(), 2);
                    assert_eq!(
                        *f.primary.objects.lock().unwrap(),
                        *f.replica.objects.lock().unwrap()
                    );
                    assert_eq!(
                        *f.primary.object_schemas.lock().unwrap(),
                        *f.replica.object_schemas.lock().unwrap()
                    );
                    assert_eq!(f.replica.records.lock().unwrap().len(), 1);
                    assert_eq!(f.replica.rows.lock().unwrap().len(), 1);
                    assert_eq!(f.replica.files.lock().unwrap().len(), 1);
                    assert_eq!(
                        f.replica.control.writes.lock().unwrap().as_slice(),
                        &[1, 2, 3, 4, 5, 6, 7, 8]
                    );
                    assert_eq!(
                        f.replica
                            .control
                            .finish_positions
                            .lock()
                            .unwrap()
                            .as_slice(),
                        &[8]
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("save_retries_duplicate", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.write_object(&document("finished")).await.unwrap();
                    f.primary.control.flush_failures.store(1, Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    assert_eq!(f.storage.status().saved.sequence, 0);
                    let saved = f.storage.flush().await.unwrap();
                    assert_eq!(saved.sequence, 4);
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 2);
                    let mut forged = f.primary.objects.lock().unwrap()[&2].clone();
                    forged.value = Objects::encode(&document("different"), limits().bytes).unwrap();
                    assert!(matches!(
                        f.primary
                            .write(
                                harmos::storage::StorageBatch::new(
                                    &[harmos::storage::StoragePart::Objects(&[forged])],
                                    limits().bytes,
                                )
                                .unwrap()
                            )
                            .await,
                        Err(harmos::Error::Backend {
                            kind: harmos::BackendFailure::Conflict,
                            ..
                        })
                    ));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("objects_never_advance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    f.storage.write_object(&document("running")).await.unwrap();
                    f.primary.control.invalid_page.store(true, Ordering::SeqCst);
                    let saved = f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let mut reader = f.storage.query_in(&view, revisions()).unwrap();
                    assert!(reader.next_page().await.is_err());
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key(),
                                ObjectVersion::Latest,
                                limits().bytes
                            )
                            .await
                            .is_err()
                    );
                    assert!(
                        f.storage
                            .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                            .await
                            .is_err()
                    );
                    assert!(f.replica.objects.lock().unwrap().is_empty());
                    f.primary
                        .control
                        .invalid_page
                        .store(false, Ordering::SeqCst);
                    assert_eq!(
                        reader.next_page().await.unwrap().unwrap()[0]
                            .position
                            .sequence,
                        2
                    );
                    f.storage
                        .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                        .await
                        .unwrap();
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("wait_never_admits", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let config = StorageSettings {
                        queue_capacity: NonZeroUsize::new(1).unwrap(),
                        ..StorageSettings::default()
                    };
                    let f = Fixture::new(config).await;
                    f.primary.control.block_write.store(true, Ordering::SeqCst);
                    f.storage.write_object(&document("running")).await.unwrap();
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(20),
                            f.storage.write_object(&document("cancelled"))
                        )
                        .await
                        .is_err()
                    );
                    assert_eq!(f.storage.status().accepted.sequence, 2);
                    f.primary.control.block_write.store(false, Ordering::SeqCst);
                    f.primary.control.write_release.notify_one();
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("admission_enforce_bounds", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let admitted = f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key(),
                                ObjectVersion::Latest,
                                NonZeroUsize::MIN
                            )
                            .await
                            .is_err()
                    );
                    let mut foreign = serde_json::to_value(&admitted.version).unwrap();
                    foreign["history"]["epoch"] = serde_json::json!("foreign");
                    let foreign = serde_json::from_value(foreign).unwrap();
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key(),
                                ObjectVersion::Exact(foreign),
                                limits().bytes
                            )
                            .await
                            .is_err()
                    );
                    let future = f
                        .storage
                        .write_object(&document("future"))
                        .await
                        .unwrap()
                        .version;
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key(),
                                ObjectVersion::Exact(future),
                                limits().bytes
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    let too_large = document(&"x".repeat(5 * 1024 * 1024));
                    assert!(f.storage.write_object(&too_large).await.is_err());
                    assert_eq!(f.storage.status().accepted.sequence, 4);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("object_flush_fixed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let first = f.storage.write_object(&document("running")).await.unwrap();
                    f.primary.control.block_write.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let flush = tokio::spawn(async move { storage.flush().await });
                    f.primary.control.write_entered.notified().await;
                    let second = f.storage.write_object(&document("finished")).await.unwrap();
                    f.primary.control.block_write.store(false, Ordering::SeqCst);
                    f.primary.control.write_release.notify_one();
                    assert_eq!(flush.await.unwrap().unwrap(), first.position);
                    assert_eq!(f.storage.status().accepted, second.position);
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 1);
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 2);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("read_releases_memory", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let before = f.storage.status().read_bytes;
                    f.primary.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let key = key();
                        let read = f.storage.read_object_in::<Document>(
                            &view,
                            &key,
                            ObjectVersion::Latest,
                            limits().bytes,
                        );
                        tokio::pin!(read);
                        tokio::select! {
                            _ = f.primary.control.read_entered.notified() => {},
                            result = &mut read => panic!("read unexpectedly completed: {result:?}"),
                        }
                        assert!(f.storage.status().read_bytes > before);
                    }
                    assert_eq!(f.storage.status().read_bytes, before);
                    f.primary.control.block_read.store(false, Ordering::SeqCst);
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &key(),
                                ObjectVersion::Latest,
                                limits().bytes
                            )
                            .await
                            .unwrap()
                            .is_some()
                    );
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cannot_advance_replica", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::with_replica(StorageSettings::default(), true).await;
                    let first = f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.write_object(&document("finished")).await.unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    f.replica.control.open_entered.notified().await;
                    f.primary
                        .objects
                        .lock()
                        .unwrap()
                        .remove(&first.position.sequence);
                    f.primary
                        .control
                        .lost_objects_through
                        .store(first.position.sequence as usize, Ordering::SeqCst);
                    f.replica.control.block_open.store(false, Ordering::SeqCst);
                    f.replica.control.open_release.notify_one();
                    assert!(
                        f.storage
                            .wait_replicas(&saved, &[Identifier::new("remote").unwrap()])
                            .await
                            .is_err()
                    );
                    assert_eq!(f.replica.saved.load(Ordering::SeqCst), 0);
                    assert!(f.replica.objects.lock().unwrap().is_empty());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("follower_decodes_saved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    f.storage.write_object(&document("finished")).await.unwrap();
                    let mut reader = f.storage.follow(revisions()).await.unwrap();
                    for status in ["running", "finished"] {
                        let page = reader.next_page().await.unwrap();
                        assert_eq!(page.len(), 1);
                        assert_eq!(page[0].value, document(status));
                    }
                    assert_eq!(f.storage.cursor_saved().sequence, 2);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("object_replay_excludes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    f.storage.write_object(&document("running")).await.unwrap();
                    f.storage.flush().await.unwrap();
                    f.storage.write_object(&document("finished")).await.unwrap();
                    let mut replay = f.storage.replay(revisions()).await.unwrap();
                    f.storage.write_object(&document("later")).await.unwrap();
                    for status in ["running", "finished"] {
                        let page = replay.next_page().await.unwrap().unwrap();
                        assert_eq!(page[0].value, document(status));
                    }
                    assert!(replay.next_page().await.unwrap().is_none());
                    assert_eq!(replay.boundary().sequence, 4);
                    drop(replay);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("heterogeneous_group_captures", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let current = document("current");
                    let legacy = LegacyDocument {
                        id: "legacy".into(),
                        status: "old".into(),
                    };
                    let receipts = f
                        .storage
                        .write_object_group(
                            NonZeroUsize::new(64 * 1024).unwrap(),
                            NonZeroUsize::new(2).unwrap(),
                            |group| {
                                assert_eq!(f.storage.cursor_accepted().sequence, 0);
                                assert_eq!(f.storage.status().pending_batches, 1);
                                group.write(&current)?;
                                group.write(&legacy)
                            },
                        )
                        .await
                        .unwrap();
                    assert_eq!(receipts[0].position.sequence, 2);
                    assert_eq!(receipts[1].position.sequence, 4);
                    assert_eq!(f.storage.status().pending_batches, 1);
                    assert!(f.primary.schemas.lock().unwrap().is_empty());
                    assert!(f.primary.objects.lock().unwrap().is_empty());
                    f.storage.flush_until(&receipts[1].position).await.unwrap();
                    {
                        let schemas = f.primary.schemas.lock().unwrap();
                        assert_eq!(
                            schemas[&1].schema,
                            harmos::schema::graph::Schema::from_declaration(
                                Document::DEFINITION,
                                &harmos::config::SchemaSettings::default()
                            )
                            .unwrap()
                        );
                        assert_eq!(
                            schemas[&3].schema,
                            harmos::schema::graph::Schema::from_declaration(
                                LegacyDocument::DEFINITION,
                                &harmos::config::SchemaSettings::default()
                            )
                            .unwrap()
                        );
                    }
                    assert_eq!(*f.primary.control.writes.lock().unwrap(), [1, 2, 3, 4]);
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 2);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("group_preserves_capture", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().pending_bytes;
                    let failure = f
                        .storage
                        .write_object_group(
                            NonZeroUsize::new(64 * 1024).unwrap(),
                            NonZeroUsize::new(2).unwrap(),
                            |group| group.write(&document("uncommitted")),
                        )
                        .await;
                    assert!(failure.is_err());
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().pending_batches, 0);
                    assert_eq!(f.storage.status().pending_bytes, baseline);
                    let receipt = f
                        .storage
                        .write_object(&document("committed"))
                        .await
                        .unwrap();
                    assert_eq!(receipt.position.sequence, 2);
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.primary.objects.lock().unwrap().len(), 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("panicking_group_releases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().pending_bytes;
                    let storage = f.storage.clone();
                    let panic = tokio::spawn(async move {
                        storage
                            .write_object_group(
                                NonZeroUsize::new(64 * 1024).unwrap(),
                                NonZeroUsize::MIN,
                                |_| panic!("producer callback failed"),
                            )
                            .await
                    })
                    .await
                    .unwrap_err();
                    assert!(panic.is_panic());
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().pending_batches, 0);
                    assert_eq!(f.storage.status().pending_bytes, baseline);
                    let receipt = f
                        .storage
                        .write_object(&document("after panic"))
                        .await
                        .unwrap();
                    assert_eq!(receipt.position.sequence, 2);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("split_group_retry", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let config = StorageSettings {
                        target_batch_bytes: NonZeroUsize::new(512).unwrap(),
                        max_batch_bytes: NonZeroUsize::new(512).unwrap(),
                        ..StorageSettings::default()
                    };
                    let f = Fixture::new(config).await;
                    let receipts = f
                        .storage
                        .write_object_group(
                            NonZeroUsize::new(64 * 1024).unwrap(),
                            NonZeroUsize::new(2).unwrap(),
                            |group| {
                                group.write(&document("first"))?;
                                group.write(&document("second"))
                            },
                        )
                        .await
                        .unwrap();
                    f.primary.control.flush_failures.store(1, Ordering::SeqCst);
                    assert!(f.storage.flush_until(&receipts[1].position).await.is_err());
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    assert_eq!(f.storage.cursor_accepted(), receipts[1].position);
                    f.storage.flush_until(&receipts[1].position).await.unwrap();
                    assert_eq!(f.storage.cursor_saved(), receipts[1].position);
                    assert!(f.primary.control.write_sizes.lock().unwrap().len() > 1);
                    {
                        let objects = f.primary.objects.lock().unwrap();
                        assert_eq!(objects.len(), 2);
                        assert_eq!(objects[&2].version, receipts[0].version);
                        assert_eq!(objects[&4].version, receipts[1].version);
                    }
                    assert_eq!(f.primary.schemas.lock().unwrap().len(), 2);
                    f.finish().await;
                });
        });
    }
}

/* === Tests === */
