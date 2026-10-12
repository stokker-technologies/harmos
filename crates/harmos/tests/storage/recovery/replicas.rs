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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Occupied;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::services::Resident<Bank> for Occupied {
    type Error = Error;

    async fn attempt(
        &mut self,
        mut context: harmos::services::Service<Bank>,
    ) -> Result<(), harmos::services::Fault<Error>> {
        context.until_stopping(std::future::pending::<()>()).await;
        Ok(())
    }
    async fn finalize(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn replica(targets: &mut Vec<ReplicaTarget>, id: &str) -> Arc<Data> {
    let backend = Arc::new(Data::new(history()));
    targets.push(ReplicaTarget {
        id: Identifier::new(id).unwrap(),
        backend: backend.clone(),
    });
    backend
}

async fn wait(
    storage: &StorageManager,
    id: &str,
    saved: &StorageCursor,
) -> Result<Vec<ReplicaStatus>, Error> {
    storage
        .wait_replicas(saved, &[Identifier::new(id).unwrap()])
        .await
}

async fn failure(storage: &StorageManager) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while storage.replicas()[0].failure.is_none() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("copies_saved_application", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy, targets).await;
                    f.storage.write_schema(&schema(), &[]).await.unwrap();
                    write(&f.storage, "record").await;
                    let key = schema();
                    let item = Identifier::new("sample").unwrap();
                    let stream = Identifier::new("signal").unwrap();
                    f.storage
                        .write_stream(&[StreamWrite {
                            stream: &stream,
                            sequence: 7,
                            record: record(&key, &item, b"row"),
                        }])
                        .await
                        .unwrap();
                    f.storage.write_recovery(&[b"local-only"]).await.unwrap();
                    assert!(target.records.lock().unwrap().is_empty());
                    assert!(target.rows.lock().unwrap().is_empty());
                    f.storage.flush_recovery().await.unwrap();
                    assert_eq!(f.storage.cursor_saved().sequence, 3);
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    assert_eq!(target.control.writes.lock().unwrap().as_slice(), &[1, 2, 3]);
                    assert_eq!(target.records.lock().unwrap().len(), 1);
                    assert_eq!(target.rows.lock().unwrap()[&3].sequence, 7);
                    assert_eq!(target.schemas.lock().unwrap().len(), 1);
                    assert_eq!(f.storage.status().recovery_saved.sequence, 1);
                    f.finish().await;
                    assert!(!target.control.open.load(Ordering::SeqCst));
                });
        });
    }
    {
        scenario_runtime::check("replicas_progress_independently", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let slow = replica(&mut targets, "slow");
                    let fast = replica(&mut targets, "fast");
                    slow.control.block_write.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let first = f.storage.flush().await.unwrap();
                    slow.control.write_entered.notified().await;
                    wait(&f.storage, "fast", &first).await.unwrap();
                    write(&f.storage, "two").await;
                    let second = f.storage.flush().await.unwrap();
                    assert_eq!(second.sequence, 2);
                    wait(&f.storage, "fast", &second).await.unwrap();
                    assert_eq!(fast.records.lock().unwrap().len(), 2);
                    slow.control.block_write.store(false, Ordering::SeqCst);
                    slow.control.write_release.notify_one();
                    wait(&f.storage, "slow", &second).await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("separate_retries_resubmission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.permanent.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    let owner_bytes = f.storage.status().pending_bytes;
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    assert!(wait(&f.storage, "remote", &saved).await.is_err());
                    assert!(f.storage.status().failure.is_none());
                    assert!(f.storage.replicas()[0].failure.is_some());
                    assert!(f.storage.status().pending_bytes > owner_bytes);
                    target.control.permanent.store(false, Ordering::SeqCst);
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    assert_eq!(target.records.lock().unwrap().len(), 1);
                    assert!(
                        target
                            .control
                            .writes
                            .lock()
                            .unwrap()
                            .iter()
                            .all(|position| *position == 1)
                    );
                    let views = f.data.control.view_ids.lock().unwrap().clone();
                    assert!(views.len() > 1);
                    assert_eq!(
                        views
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len(),
                        views.len()
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("uncertainty_checks_receipt", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.flush_failures.store(1, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    failure(&f.storage).await;
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    assert_eq!(target.control.writes.lock().unwrap().as_slice(), &[1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("files_stream_reconcile", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    let mut targets = Vec::new();
                    policy.file_chunk_bytes = NonZeroUsize::new(17).unwrap();
                    let target = replica(&mut targets, "remote");
                    target.control.finish_failures.store(1, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    let bytes = vec![11; 4096];
                    let metadata = FileMetadata {
                        id: Identifier::new("file").unwrap(),
                        scope: None,
                        media_type: "application/octet-stream".into(),
                        expected_length: None,
                        expected_sha256: None,
                    };
                    let file = f.storage.write_file(metadata, &bytes).await.unwrap();
                    let saved = f.storage.flush().await.unwrap();
                    failure(&f.storage).await;
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    assert_eq!(target.files.lock().unwrap()[&file.position.sequence], file);
                    assert!(target.staged.lock().unwrap().is_empty());
                    assert!(
                        target
                            .control
                            .chunk_sizes
                            .lock()
                            .unwrap()
                            .iter()
                            .all(|size| *size <= 17)
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("replica_close_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    target.control.fail_close.store(true, Ordering::SeqCst);
                    assert!(f.storage.close().await.is_err());
                    assert!(f.data.control.open.load(Ordering::SeqCst));
                    target.control.fail_close.store(false, Ordering::SeqCst);
                    f.storage.close().await.unwrap();
                    assert!(f.storage.replicas()[0].closed);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("shutdown_wait_retryable", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    let mut targets = Vec::new();
                    policy.replication_shutdown = Duration::from_millis(10);
                    policy.io_timeout = Duration::from_secs(60);
                    let target = replica(&mut targets, "remote");
                    target.control.block_flush.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    f.storage.flush().await.unwrap();
                    target.control.flush_entered.notified().await;
                    assert!(f.storage.close().await.is_err());
                    assert!(f.data.control.open.load(Ordering::SeqCst));
                    target.control.block_flush.store(false, Ordering::SeqCst);
                    target.control.flush_release.notify_one();
                    f.storage.close().await.unwrap();
                    assert_eq!(f.storage.replicas()[0].saved.as_ref().unwrap().sequence, 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_retains_replication", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let policy = config();
    let mut targets = Vec::new();
    let target = replica(&mut targets, "remote");
    target.control.block_write.store(true, Ordering::SeqCst);
    let f = Fixture::with_replicas(policy, targets).await;
    write(&f.storage, "one").await;
    let saved = f.storage.flush().await.unwrap();
    target.control.write_entered.notified().await;
    {
        let waiting = wait(&f.storage, "remote", &saved);
        tokio::pin!(waiting);
        tokio::select! { biased; _ = &mut waiting => panic!("target is blocked"), _ = tokio::task::yield_now() => {} }
    }
    target.control.block_write.store(false, Ordering::SeqCst);
    target.control.write_release.notify_one();
    wait(&f.storage, "remote", &saved).await.unwrap();
    assert_eq!(target.control.writes.lock().unwrap().as_slice(), &[1]);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("reopened_manager_reconciles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy.clone(), targets.clone()).await;
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    f.storage.close().await.unwrap();
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: f.data.clone(),
                            recovery: f.recovery.clone(),
                            replicas: targets,
                        },
                        policy,
                    )
                    .unwrap();
                    storage.start(&f.runtime.services).await.unwrap();
                    wait(&storage, "remote", &saved).await.unwrap();
                    write(&storage, "two").await;
                    let saved = storage.flush().await.unwrap();
                    wait(&storage, "remote", &saved).await.unwrap();
                    assert_eq!(target.control.writes.lock().unwrap().as_slice(), &[1, 2]);
                    storage.close().await.unwrap();
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
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy, targets).await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    write(&f.storage, "one").await;
                    f.storage.flush().await.unwrap();
                    f.data.control.view_entered.notified().await;
                    f.data.records.lock().unwrap().clear();
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    failure(&f.storage).await;
                    assert_eq!(f.storage.replicas()[0].saved.as_ref().unwrap().sequence, 0);
                    assert!(target.control.flushes.lock().unwrap().is_empty());
                    assert!(f.storage.status().failure.is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("wrong_target_receipt", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.wrong_receipt.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    failure(&f.storage).await;
                    assert_eq!(f.storage.replicas()[0].saved.as_ref().unwrap().sequence, 0);
                    target.control.wrong_receipt.store(false, Ordering::SeqCst);
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancelled_close_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let policy = config();
    let mut targets = Vec::new();
    let target = replica(&mut targets, "remote");
    target.control.block_write.store(true, Ordering::SeqCst);
    let f = Fixture::with_replicas(policy, targets).await;
    write(&f.storage, "one").await;
    f.storage.flush().await.unwrap();
    target.control.write_entered.notified().await;
    {
        let closing = f.storage.close();
        tokio::pin!(closing);
        tokio::select! { biased; _ = &mut closing => panic!("target I/O is still owned"), _ = tokio::task::yield_now() => {} }
    }
    assert!(f.data.control.open.load(Ordering::SeqCst));
    target.control.block_write.store(false, Ordering::SeqCst);
    target.control.write_release.notify_one();
    f.storage.close().await.unwrap();
    assert!(f.storage.replicas()[0].closed);
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("continuations_preserve_capture", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    let mut targets = Vec::new();
                    policy.replication_limits.rows = NonZeroUsize::new(1).unwrap();
                    let target = replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy, targets).await;
                    for n in 0..40 {
                        write(&f.storage, &format!("item-{n}")).await;
                    }
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    assert_eq!(
                        *target.control.writes.lock().unwrap(),
                        (1..=40).collect::<Vec<_>>()
                    );
                    assert_eq!(target.records.lock().unwrap().len(), 40);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("flush_never_extend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.block_flush.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let first = f.storage.flush().await.unwrap();
                    target.control.flush_entered.notified().await;
                    write(&f.storage, "two").await;
                    let second = f.storage.flush().await.unwrap();
                    target.control.flush_release.notify_one();
                    wait(&f.storage, "remote", &first).await.unwrap();
                    target.control.flush_entered.notified().await;
                    assert_eq!(f.storage.replicas()[0].saved.as_ref().unwrap(), &first);
                    target.control.block_flush.store(false, Ordering::SeqCst);
                    target.control.flush_release.notify_one();
                    wait(&f.storage, "remote", &second).await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cleanup_preserves_existing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    let mut targets = Vec::new();
                    policy.replication_shutdown = Duration::from_millis(10);
                    let target = replica(&mut targets, "remote");
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .runtime()
                        .await
                        .unwrap();
                    let name = "harmos-storage-replica-remote";
                    runtime
                        .services
                        .register(
                            harmos::services::ServiceDefinition {
                                id: name.into(),
                                policy: harmos::services::ServicePolicy::default(),
                            },
                            |_: Arc<()>| Occupied,
                        )
                        .unwrap();
                    let _lease = runtime
                        .services
                        .activate(name, &runtime.services.root_scope())
                        .await
                        .unwrap();
                    let data = Arc::new(Data::new(history()));
                    let recovery = Arc::new(Recovery::new(history()));
                    let storage = StorageManager::new(
                        StorageBackends {
                            primary: data.clone(),
                            recovery,
                            replicas: targets,
                        },
                        policy,
                    )
                    .unwrap();
                    assert!(storage.start(&runtime.services).await.is_err());
                    storage.close().await.unwrap();
                    assert!(
                        runtime
                            .services
                            .status(&harmos::services::ServiceKey::root(name))
                            .is_some()
                    );
                    assert_eq!(target.control.closes.load(Ordering::SeqCst), 0);
                    assert_eq!(storage.status().pending_bytes, 0);
                    assert!(!data.control.open.load(Ordering::SeqCst));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("target_open_reconciles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.fail_open.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    failure(&f.storage).await;
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    target.control.fail_open.store(false, Ordering::SeqCst);
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    assert_eq!(target.records.lock().unwrap().len(), 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("receipt_never_erase", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let first = f.storage.flush().await.unwrap();
                    wait(&f.storage, "remote", &first).await.unwrap();
                    target.control.regressed_saved.store(true, Ordering::SeqCst);
                    write(&f.storage, "two").await;
                    let second = f.storage.flush().await.unwrap();
                    failure(&f.storage).await;
                    assert_eq!(f.storage.replicas()[0].saved.as_ref().unwrap(), &first);
                    target
                        .control
                        .regressed_saved
                        .store(false, Ordering::SeqCst);
                    wait(&f.storage, "remote", &second).await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("source_pin_release", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    replica(&mut targets, "remote");
                    let f = Fixture::with_replicas(policy, targets).await;
                    f.data.control.fail_release.store(true, Ordering::SeqCst);
                    write(&f.storage, "one").await;
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "remote", &saved).await.unwrap();
                    failure(&f.storage).await;
                    assert!(f.storage.close().await.is_err());
                    assert!(f.data.control.open.load(Ordering::SeqCst));
                    f.data.control.fail_release.store(false, Ordering::SeqCst);
                    f.storage.close().await.unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("replicas_share_capacity", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    let mut targets = Vec::new();
                    policy.queue_capacity_bytes = NonZeroUsize::new(6 * 1024 * 1024).unwrap();
                    policy.target_batch_bytes = policy.queue_capacity_bytes;
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    // Five collections at 48 KiB preserve this test's original transfer budget.
                    policy.replication_limits.bytes = NonZeroUsize::new(48 * 1024).unwrap();
                    let first = replica(&mut targets, "first");
                    let second = replica(&mut targets, "second");
                    first.control.block_write.store(true, Ordering::SeqCst);
                    second.control.block_write.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    f.storage.flush().await.unwrap();
                    tokio::select! {
                        _ = first.control.write_entered.notified() => {},
                        _ = second.control.write_entered.notified() => {},
                    }
                    assert!(f.storage.status().pending_bytes <= 6 * 1024 * 1024);
                    write(&f.storage, "two").await;
                    let saved = f.storage.flush().await.unwrap();
                    first.control.block_write.store(false, Ordering::SeqCst);
                    second.control.block_write.store(false, Ordering::SeqCst);
                    first.control.write_release.notify_one();
                    second.control.write_release.notify_one();
                    wait(&f.storage, "first", &saved).await.unwrap();
                    wait(&f.storage, "second", &saved).await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cleanup_releases_source", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let policy = config();
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.fail_finish.store(true, Ordering::SeqCst);
                    target.control.fail_abort.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    let metadata = FileMetadata {
                        id: Identifier::new("cleanup").unwrap(),
                        scope: None,
                        media_type: "application/octet-stream".into(),
                        expected_length: None,
                        expected_sha256: None,
                    };
                    f.storage
                        .write_file(metadata, b"owned bytes")
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    failure(&f.storage).await;
                    assert!(f.data.control.releases.load(Ordering::SeqCst) > 0);
                    assert!(!target.staged.lock().unwrap().is_empty());
                    assert!(f.storage.close().await.is_err());
                    assert!(f.storage.status().pending_bytes > 0);
                    target.control.fail_finish.store(false, Ordering::SeqCst);
                    target.control.fail_abort.store(false, Ordering::SeqCst);
                    f.storage.close().await.unwrap();
                    assert!(target.staged.lock().unwrap().is_empty());
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cleanup_keeps_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.io_timeout = Duration::from_millis(20);
                    let mut targets = Vec::new();
                    let target = replica(&mut targets, "remote");
                    target.control.fail_finish.store(true, Ordering::SeqCst);
                    target.control.block_abort.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    f.storage
                        .write_file(
                            FileMetadata {
                                id: Identifier::new("stalled-cleanup").unwrap(),
                                scope: None,
                                media_type: "application/octet-stream".into(),
                                expected_length: None,
                                expected_sha256: None,
                            },
                            b"owned bytes",
                        )
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    target.control.abort_entered.notified().await;
                    tokio::time::timeout(Duration::from_secs(1), async {
                        while f.data.control.releases.load(Ordering::SeqCst) == 0 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    tokio::time::sleep(Duration::from_millis(60)).await;
                    assert_eq!(target.control.aborts.load(Ordering::SeqCst), 1);
                    assert!(f.storage.replicas()[0].failure.is_none());
                    assert!(!target.staged.lock().unwrap().is_empty());
                    assert!(f.storage.status().pending_bytes > 0);
                    target.control.block_abort.store(false, Ordering::SeqCst);
                    target.control.abort_release.notify_one();
                    failure(&f.storage).await;
                    f.storage.close().await.unwrap();
                    assert!(target.staged.lock().unwrap().is_empty());
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("replica_pages_split", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    let mut targets = Vec::new();
                    policy.max_batch_bytes = NonZeroUsize::new(1024).unwrap();
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let target = replica(&mut targets, "bounded");
                    let f = Fixture::with_replicas(policy, targets).await;
                    let key = schema();
                    let item = Identifier::new("row").unwrap();
                    let payload = vec![7; 100];
                    let records: Vec<_> = (0..8).map(|_| record(&key, &item, &payload)).collect();
                    let accepted = f.storage.write_records(&records).await.unwrap();
                    f.storage.flush_until(&accepted).await.unwrap();
                    wait(&f.storage, "bounded", &accepted).await.unwrap();
                    assert_eq!(
                        *target.control.writes.lock().unwrap(),
                        (1..=8).collect::<Vec<_>>()
                    );
                    let bytes = target.control.write_bytes.lock().unwrap().clone();
                    assert!(bytes.len() > 1 && bytes.iter().all(|count| *count <= 1024));
                    assert_eq!(target.records.lock().unwrap().len(), 8);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("lag_refuses_groups", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.replica_lag_positions = std::num::NonZeroU64::new(2).unwrap();
                    let mut targets = Vec::new();
                    let slow = replica(&mut targets, "slow");
                    let fast = replica(&mut targets, "fast");
                    slow.control.permanent.store(true, Ordering::SeqCst);
                    let f = Fixture::with_replicas(policy, targets).await;
                    write(&f.storage, "one").await;
                    let key = schema();
                    let item = Identifier::new("group").unwrap();
                    assert!(matches!(
                        f.storage
                            .write_records(&[
                                record(&key, &item, b"one"),
                                record(&key, &item, b"two"),
                            ])
                            .await,
                        Err(Error::ResourceExhausted {
                            resource: "replica retained history"
                        })
                    ));
                    assert_eq!(f.storage.status().accepted.sequence, 1);
                    write(&f.storage, "two").await;
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "fast", &saved).await.unwrap();
                    failure(&f.storage).await;
                    assert_eq!(fast.records.lock().unwrap().len(), 2);
                    let mut file = f
                        .storage
                        .create_file(FileMetadata {
                            id: Identifier::new("lagged-file").unwrap(),
                            scope: None,
                            media_type: "application/octet-stream".into(),
                            expected_length: None,
                            expected_sha256: None,
                        })
                        .await
                        .unwrap();
                    file.write_all(b"retained staging").await.unwrap();
                    assert!(matches!(
                        file.finish().await,
                        Err(Error::ResourceExhausted {
                            resource: "replica retained history"
                        })
                    ));
                    assert_eq!(file.length(), 16);
                    assert_eq!(f.storage.status().accepted.sequence, 2);
                    assert!(f.storage.status().failure.is_none());
                    f.storage
                        .write_recovery(&[b"recovery progresses"])
                        .await
                        .unwrap();
                    f.storage.flush_recovery().await.unwrap();
                    slow.control.permanent.store(false, Ordering::SeqCst);
                    wait(&f.storage, "slow", &saved).await.unwrap();
                    let reference = file.finish().await.unwrap();
                    assert_eq!(reference.position.sequence, 3);
                    let saved = f.storage.flush().await.unwrap();
                    wait(&f.storage, "slow", &saved).await.unwrap();
                    wait(&f.storage, "fast", &saved).await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("missing_replica_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.replica_lag_positions = std::num::NonZeroU64::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    for item in ["one", "two", "three"] {
                        write(&f.storage, item).await;
                    }
                    assert_eq!(f.storage.flush().await.unwrap().sequence, 3);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("rejects_duplicate_owners", || {
            let policy = config();
            let mut targets = Vec::new();
            let target = replica(&mut targets, "remote");
            let recovery = Arc::new(Recovery::new(history()));
            assert!(
                StorageManager::new(
                    StorageBackends {
                        primary: target.clone(),
                        recovery: recovery.clone(),
                        replicas: targets.clone()
                    },
                    policy.clone()
                )
                .is_err()
            );
            targets.push(ReplicaTarget {
                id: Identifier::new("another").unwrap(),
                backend: target,
            });
            assert!(
                StorageManager::new(
                    StorageBackends {
                        primary: Arc::new(Data::new(history())),
                        recovery: recovery.clone(),
                        replicas: targets
                    },
                    policy
                )
                .is_err()
            );
            let mut policy = config();
            let mut targets = Vec::new();
            replica(&mut targets, "remote");
            policy.replication_limits.bytes = NonZeroUsize::new(usize::MAX).unwrap();
            assert!(
                StorageManager::new(
                    StorageBackends {
                        primary: Arc::new(Data::new(history())),
                        recovery,
                        replicas: targets
                    },
                    policy
                )
                .is_err()
            );
        });
    }
}

/* === Tests === */
