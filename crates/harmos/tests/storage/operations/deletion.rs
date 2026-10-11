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

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{Fixture, Lanes, Operations, config};
use harmos::Identifier;
use harmos::storage::operations::{DeleteFile, DeleteFileRequest};
use harmos::storage::{FileMetadata, MutationId, ReplicaTarget, StagedFile, StorageBackend};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn metadata(id: &str) -> FileMetadata {
    FileMetadata {
        id: Identifier::new(id).unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    }
}
fn request(f: &Fixture, id: &str) -> DeleteFileRequest {
    DeleteFileRequest {
        mutation: MutationId {
            window: f.storage.mutation_window().unwrap().id,
            key: Identifier::new("delete").unwrap(),
        },
        file: StagedFile {
            history: f.data.history.clone(),
            id: Identifier::new(id).unwrap(),
        },
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("deletion_rejects_producers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f.storage.produce_file(metadata("terminal")).await.unwrap();
                    producer.write_all(b"abcdef").await.unwrap();
                    let request = request(&f, "terminal");
                    assert!(
                        f.storage
                            .delete_file_once(&request.mutation, &request.file)
                            .await
                            .is_err()
                    );
                    producer.finish().await.unwrap();
                    let mut reader = f.storage.follow_file(&request.file, 0).await.unwrap();
                    let accepted = reader
                        .next_chunk(NonZeroUsize::new(2).unwrap())
                        .await
                        .unwrap()
                        .unwrap();
                    let deleted = f
                        .storage
                        .delete_file_once(&request.mutation, &request.file)
                        .await
                        .unwrap();
                    assert_eq!(deleted, request.file);
                    assert_eq!(accepted.as_bytes(), b"ab");
                    assert!(reader.next_chunk(NonZeroUsize::MIN).await.is_err());
                    assert!(f.storage.follow_file(&request.file, 0).await.is_err());
                    assert_eq!(
                        f.storage
                            .delete_file_once(&request.mutation, &request.file)
                            .await
                            .unwrap(),
                        deleted
                    );
                    let mut query = f
                        .storage
                        .query(harmos::storage::FileQuery::new())
                        .await
                        .unwrap();
                    assert!(query.next_page().await.unwrap().is_none());
                    drop(query);
                    assert!(f.data.files.lock().unwrap().is_empty());
                    assert!(f.data.blobs.lock().unwrap().is_empty());
                    assert!(f.data.create_file(&metadata("terminal")).await.is_err());
                    let other = StagedFile {
                        id: Identifier::new("other").unwrap(),
                        ..request.file.clone()
                    };
                    assert!(
                        f.storage
                            .delete_file_once(&request.mutation, &other)
                            .await
                            .is_err()
                    );
                    drop(accepted);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("abandoned_missing_files", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f.storage.produce_file(metadata("partial")).await.unwrap();
                    producer.write_all(b"partial").await.unwrap();
                    producer.abort().await.unwrap();
                    let mut request = request(&f, "partial");
                    f.storage
                        .delete_file_once(&request.mutation, &request.file)
                        .await
                        .unwrap();
                    assert!(f.data.staged.lock().unwrap().is_empty());
                    request.mutation.key = Identifier::new("missing").unwrap();
                    request.file.id = Identifier::new("missing").unwrap();
                    f.storage
                        .delete_file_once(&request.mutation, &request.file)
                        .await
                        .unwrap();
                    assert!(f.data.create_file(&metadata("missing")).await.is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("deletion_finishes_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let f = Fixture::new(config()).await;
                        f.storage
                            .write_file(metadata("cancelled"), b"bytes")
                            .await
                            .unwrap();
                        let request = request(&f, "cancelled");
                        f.data.control.block_delete.store(true, Ordering::SeqCst);
                        let mut deletion =
                            Box::pin(f.storage.delete_file_once(&request.mutation, &request.file));
                        tokio::select! {
                            biased;
                            _ = &mut deletion => panic!("deletion must wait"),
                            _ = f.data.control.delete_entered.notified() => {}
                        }
                        drop(deletion);
                        f.data.control.block_delete.store(false, Ordering::SeqCst);
                        f.data.control.delete_release.notify_one();
                        f.storage
                            .delete_file_once(&request.mutation, &request.file)
                            .await
                            .unwrap();
                        assert!(
                            f.data
                                .mutation_receipts
                                .lock()
                                .unwrap()
                                .contains_key(&request.mutation)
                        );
                        assert_eq!(f.storage.status().read_bytes, 0);
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("deletion_retries_lost", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    Operations::register(
                        &f.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(f.storage.clone()),
                    )
                    .unwrap();
                    f.storage
                        .write_file(metadata("retry"), b"bytes")
                        .await
                        .unwrap();
                    f.data.control.delete_failures.store(3, Ordering::SeqCst);
                    let request = request(&f, "retry");
                    let result = f
                        .runtime
                        .operations
                        .call::<DeleteFile>(request.clone())
                        .await
                        .unwrap();
                    assert_eq!(*result, request.file);
                    assert_eq!(f.data.control.delete_failures.load(Ordering::SeqCst), 0);
                    assert_eq!(f.data.retention_generation.load(Ordering::SeqCst), 2);
                    assert!(
                        f.data
                            .mutation_receipts
                            .lock()
                            .unwrap()
                            .contains_key(&request.mutation)
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("deletion_waits_replica", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let replica = Arc::new(crate::Data::new(crate::backend::history()));
                        let f = Fixture::with_replicas(
                            config(),
                            vec![ReplicaTarget {
                                id: Identifier::new("replica").unwrap(),
                                backend: replica.clone(),
                            }],
                        )
                        .await;
                        replica.control.block_flush.store(true, Ordering::SeqCst);
                        f.storage
                            .write_file(metadata("replicated"), b"bytes")
                            .await
                            .unwrap();
                        let request = request(&f, "replicated");
                        let mut deletion =
                            Box::pin(f.storage.delete_file_once(&request.mutation, &request.file));
                        tokio::select! {
                            biased;
                            _ = &mut deletion => panic!("deletion must wait for replica"),
                            _ = replica.control.flush_entered.notified() => {}
                        }
                        assert!(f.data.blobs.lock().unwrap().contains_key("replicated"));
                        assert!(replica.blobs.lock().unwrap().contains_key("replicated"));
                        replica.control.block_flush.store(false, Ordering::SeqCst);
                        replica.control.flush_release.notify_one();
                        deletion.await.unwrap();
                        assert!(f.data.blobs.lock().unwrap().is_empty());
                        assert!(replica.blobs.lock().unwrap().is_empty());
                        assert!(replica.deleted_files.lock().unwrap().contains("replicated"));
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("new_replica_bootstraps", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let f = Fixture::new(config()).await;
                        f.storage
                            .write_file(metadata("deleted"), b"gone")
                            .await
                            .unwrap();
                        f.storage
                            .write_file(metadata("kept"), b"retained")
                            .await
                            .unwrap();
                        let request = request(&f, "deleted");
                        f.storage
                            .delete_file_once(&request.mutation, &request.file)
                            .await
                            .unwrap();
                        let saved = f.storage.cursor_saved();
                        f.storage.close().await.unwrap();
                        f.data.control.allow_bootstrap.store(true, Ordering::SeqCst);
                        let target = Arc::new(crate::Data::new(crate::backend::history()));
                        target.control.allow_bootstrap.store(true, Ordering::SeqCst);
                        let storage = harmos::storage::StorageManager::new(
                            harmos::storage::StorageBackends {
                                primary: f.data.clone(),
                                recovery: Arc::new(crate::Recovery::new(crate::backend::history())),
                                replicas: vec![ReplicaTarget {
                                    id: Identifier::new("new").unwrap(),
                                    backend: target.clone(),
                                }],
                            },
                            config(),
                        )
                        .unwrap();
                        storage.start(&f.runtime.services).await.unwrap();
                        storage
                            .wait_replicas(&saved, &[Identifier::new("new").unwrap()])
                            .await
                            .unwrap();
                        assert_eq!(target.control.bootstrap_begins.load(Ordering::SeqCst), 1);
                        assert!(!target.blobs.lock().unwrap().contains_key("deleted"));
                        assert_eq!(target.blobs.lock().unwrap()["kept"].1, b"retained");
                        assert_eq!(target.files.lock().unwrap().len(), 1);
                        let retained = target
                            .read_retained_view(&saved, config().query_execution)
                            .await
                            .unwrap();
                        assert!(retained.coverage.complete_after > 0);
                        target.release_view(&retained.snapshot).await.unwrap();
                        storage.close().await.unwrap();
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("deletion_never_multiplies", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_retries = 3;
                    let f = Fixture::new(settings).await;
                    Operations::register(
                        &f.runtime.operations,
                        "storage",
                        Lanes::Open,
                        Arc::new(f.storage.clone()),
                    )
                    .unwrap();
                    f.storage
                        .write_file(metadata("budget"), b"bytes")
                        .await
                        .unwrap();
                    f.data.control.delete_failures.store(5, Ordering::SeqCst);
                    let request = request(&f, "budget");
                    assert!(
                        f.runtime
                            .operations
                            .call::<DeleteFile>(request.clone())
                            .await
                            .is_err()
                    );
                    assert_eq!(f.data.control.delete_failures.load(Ordering::SeqCst), 1);
                    f.data.control.delete_failures.store(0, Ordering::SeqCst);
                    f.storage
                        .delete_file_once(&request.mutation, &request.file)
                        .await
                        .unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("deletion_preserves_other", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let kept = f
                        .storage
                        .write_file(metadata("kept-first"), b"kept")
                        .await
                        .unwrap();
                    let removed = f
                        .storage
                        .write_file(metadata("removed-last"), b"gone")
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let mut pinned = f.storage.read_file(&removed).await.unwrap();
                    let chunk = pinned.next_chunk(NonZeroUsize::MIN).await.unwrap().unwrap();
                    let request = request(&f, "removed-last");
                    f.storage
                        .delete_file_once(&request.mutation, &request.file)
                        .await
                        .unwrap();
                    assert_eq!(chunk.as_bytes(), b"g");
                    assert!(pinned.next_chunk(NonZeroUsize::MIN).await.is_err());
                    assert!(f.storage.read_file(&removed).await.is_err());
                    let mut reader = f.storage.read_file(&kept).await.unwrap();
                    assert_eq!(
                        reader
                            .next_chunk(NonZeroUsize::new(4).unwrap())
                            .await
                            .unwrap()
                            .unwrap()
                            .as_bytes(),
                        b"kept"
                    );
                    drop((pinned, chunk, reader));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cannot_restore_deleted", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut producer = f
                        .storage
                        .produce_file(metadata("checkpoint"))
                        .await
                        .unwrap();
                    producer.write_all(b"partial").await.unwrap();
                    f.storage.flush().await.unwrap();
                    let checkpoint = f.data.saved_productions.lock().unwrap()["checkpoint"]
                        .0
                        .clone();
                    producer.abort().await.unwrap();
                    let request = request(&f, "checkpoint");
                    f.storage
                        .delete_file_once(&request.mutation, &request.file)
                        .await
                        .unwrap();
                    f.data.save_production(&checkpoint).await.unwrap();
                    assert!(f.data.saved_productions.lock().unwrap().is_empty());
                    assert!(f.data.staged.lock().unwrap().is_empty());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("close_settles_deletion", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
        let f = Fixture::new(config()).await;
        f.storage
            .write_file(metadata("close"), b"bytes")
            .await
            .unwrap();
        let request = request(&f, "close");
        f.data.control.block_delete.store(true, Ordering::SeqCst);
        let mut deletion = Box::pin(f.storage.delete_file_once(&request.mutation, &request.file));
        tokio::select! {
            biased;
            _ = &mut deletion => panic!("deletion must wait"),
            _ = f.data.control.delete_entered.notified() => {}
        }
        drop(deletion);
        let closing = f.storage.close();
        tokio::pin!(closing);
        tokio::select! {
            biased;
            result = &mut closing => panic!("close must wait for the blocked deletion: {result:?}"),
            // One poll of close: it must already observe the in-flight deletion.
            _ = std::future::ready(()) => {}
        }
        assert!(f.storage.status().closing);
        f.data.control.block_delete.store(false, Ordering::SeqCst);
        f.data.control.delete_release.notify_one();
        closing.await.unwrap();
        assert!(
            f.data
                .mutation_receipts
                .lock()
                .unwrap()
                .contains_key(&request.mutation)
        );
        assert!(f.data.blobs.lock().unwrap().is_empty());
        assert_eq!(f.storage.status().read_bytes, 0);
        f.runtime.stop().await.unwrap();
    })
    .await
    .unwrap();
                });
        });
    }
    {
        scenario_runtime::check("receipt_waits_backend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let replica = Arc::new(crate::Data::new(crate::backend::history()));
                        let f = Fixture::with_replicas(
                            config(),
                            vec![ReplicaTarget {
                                id: Identifier::new("replica").unwrap(),
                                backend: replica.clone(),
                            }],
                        )
                        .await;
                        f.storage
                            .write_file(metadata("partial-delete"), b"bytes")
                            .await
                            .unwrap();
                        let request = request(&f, "partial-delete");
                        replica.control.block_delete.store(true, Ordering::SeqCst);
                        replica.control.delete_failures.store(1, Ordering::SeqCst);
                        let mut deletion =
                            Box::pin(f.storage.delete_file_once(&request.mutation, &request.file));
                        tokio::select! {
                            biased;
                            _ = &mut deletion => panic!("deletion must wait"),
                            _ = replica.control.delete_entered.notified() => {}
                        }
                        assert!(f.data.blobs.lock().unwrap().is_empty());
                        assert!(replica.blobs.lock().unwrap().contains_key("partial-delete"));
                        assert!(
                            !f.data
                                .mutation_receipts
                                .lock()
                                .unwrap()
                                .contains_key(&request.mutation)
                        );
                        replica.control.block_delete.store(false, Ordering::SeqCst);
                        replica.control.delete_release.notify_one();
                        assert!(deletion.await.is_err());
                        assert!(
                            !f.data
                                .mutation_receipts
                                .lock()
                                .unwrap()
                                .contains_key(&request.mutation)
                        );
                        f.storage
                            .delete_file_once(&request.mutation, &request.file)
                            .await
                            .unwrap();
                        assert!(replica.blobs.lock().unwrap().is_empty());
                        assert!(
                            f.data
                                .mutation_receipts
                                .lock()
                                .unwrap()
                                .contains_key(&request.mutation)
                        );
                        f.finish().await;
                    })
                    .await
                    .unwrap();
                });
        });
    }
}
