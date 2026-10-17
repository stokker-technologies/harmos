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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn admission_durability_distinct() {
    let f = Fixture::new(config()).await;
    let key = schema();
    assert_eq!(
        f.storage
            .write_schema(
                &key,
                &harmos_runtime::schema::graph::Schema {
                    root: 0,
                    definitions: vec![vec![]]
                }
            )
            .await
            .unwrap()
            .sequence,
        1
    );
    let item = Identifier::new("sample").unwrap();
    let stream = Identifier::new("voltage").unwrap();
    assert_eq!(
        f.storage
            .write_records(&[record(&key, &item, &[3])])
            .await
            .unwrap()
            .sequence,
        2
    );
    assert_eq!(
        f.storage
            .write_stream(&[StreamWrite {
                stream: &stream,
                sequence: 99,
                record: record(&key, &item, &[4])
            }])
            .await
            .unwrap()
            .sequence,
        3
    );
    let recovery = f.storage.write_recovery(&[b"one", b"two"]).await.unwrap();
    assert_eq!(recovery.sequence, 2);
    assert_eq!(f.storage.status().saved.sequence, 0);
    assert_eq!(f.storage.flush().await.unwrap().sequence, 3);
    assert_eq!(f.storage.status().recovery_saved.sequence, 0);
    assert_eq!(f.storage.flush_recovery().await.unwrap().sequence, 2);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert_eq!(f.data.rows.lock().unwrap()[&3].sequence, 99);
    assert_eq!(f.recovery.entries.lock().unwrap().len(), 2);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn recovery_share_backpressure() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.block_flush.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    {
        let blocked = f.storage.write_recovery(&[b"state"]);
        tokio::pin!(blocked);
        assert!(crate::pending(blocked.as_mut()).await);
    }
    assert_eq!(f.storage.status().recovery_accepted.sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 1);
    assert_eq!(f.data.control.flushes.lock().unwrap().as_slice(), &[1]);
    assert_eq!(f.storage.status().saved.sequence, 0);
    f.data.control.block_flush.store(false, Ordering::SeqCst);
    f.data.control.flush_release.notify_one();
    assert_eq!(
        f.storage
            .write_recovery(&[b"state"])
            .await
            .unwrap()
            .sequence,
        1
    );
    f.finish().await;
}

#[tokio::test]
async fn close_retains_writes() {
    let f = Fixture::new(config()).await;
    f.data.control.permanent.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let error = f.storage.close().await.unwrap_err();
    let Error::Persistence { source } = error else {
        panic!("expected preserved persistence cause")
    };
    assert!(matches!(
        source.as_ref(),
        Error::Backend {
            kind: BackendFailure::Permanent,
            ..
        }
    ));
    assert!(f.storage.status().pending_bytes > 0);
    assert_eq!(f.storage.status().accepted.sequence, 1);
    assert_eq!(f.storage.status().saved.sequence, 0);
    assert!(f.storage.status().closing);
    assert!(f.data.control.open.load(Ordering::SeqCst));
    let key = schema();
    let item = Identifier::new("late").unwrap();
    assert!(matches!(
        f.storage.write_records(&[record(&key, &item, &[4])]).await,
        Err(Error::Closing { domain: "storage" })
    ));
    f.data.control.permanent.store(false, Ordering::SeqCst);
    f.storage.close().await.unwrap();
    assert_eq!(f.data.records.lock().unwrap().len(), 1);
    assert!(
        f.data
            .control
            .writes
            .lock()
            .unwrap()
            .iter()
            .all(|at| *at == 1)
    );
    assert_eq!(f.storage.status().saved.sequence, 1);
    f.runtime.stop().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn transient_retry_saves() {
    let f = Fixture::new(config()).await;
    f.data.control.failures.store(1, Ordering::SeqCst);
    let cursor = write(&f.storage, "one").await;
    assert!(f.storage.flush().await.is_err());

    // The Periodic Save Retries Without Another Explicit Flush.
    f.data.control.block_write.store(true, Ordering::SeqCst);
    tokio::time::advance(Duration::from_millis(100)).await;
    f.data.control.write_entered.notified().await;
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    f.storage.await_durable(&cursor).await.unwrap();
    assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), &[1, 1]);
    assert_eq!(f.storage.status().accepted.sequence, 1);
    f.finish().await;
}

#[tokio::test]
async fn backend_close_failure() {
    let f = Fixture::new(config()).await;
    f.recovery.control.fail_close.store(true, Ordering::SeqCst);
    assert!(f.storage.close().await.is_err());
    assert!(!f.data.control.open.load(Ordering::SeqCst));
    assert!(f.recovery.control.open.load(Ordering::SeqCst));
    assert!(!f.storage.status().closed);
    f.recovery.control.fail_close.store(false, Ordering::SeqCst);
    f.storage.close().await.unwrap();
    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 1);
    assert_eq!(f.recovery.control.closes.load(Ordering::SeqCst), 2);
    f.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn partial_open_remains() {
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime()
        .await
        .unwrap();
    let data = Arc::new(Data::new(history()));
    let recovery = Arc::new(Recovery::new(history()));
    recovery.control.fail_open.store(true, Ordering::SeqCst);
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        config(),
    )
    .unwrap();
    assert!(storage.start(&runtime).await.is_err());
    assert!(data.control.open.load(Ordering::SeqCst));
    storage.close().await.unwrap();
    assert!(!data.control.open.load(Ordering::SeqCst));
    assert!(!recovery.control.open.load(Ordering::SeqCst));
    assert!(
        runtime
            .operations
            .resident(&harmos_runtime::operations::resident::Key::root("harmos-storage"))
            .and_then(|item| item.health)
            .is_none()
    );
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_close_retains() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.block_flush.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let key = schema();
    let item = Identifier::new("blocked").unwrap();
    let inputs = [record(&key, &item, &[1])];
    let producer = f.storage.write_records(&inputs);
    tokio::pin!(producer);
    assert!(crate::pending(producer.as_mut()).await, "queue is full");
    {
        let close = f.storage.close();
        tokio::pin!(close);
        tokio::select! { biased; _ = &mut close => panic!("flush is blocked"), _ = f.data.control.flush_entered.notified() => {} }
    }
    assert!(matches!(
        producer.await,
        Err(Error::Closing { domain: "storage" })
    ));
    f.data.control.block_flush.store(false, Ordering::SeqCst);
    f.data.control.flush_release.notify_one();
    f.storage.close().await.unwrap();
    assert_eq!(f.data.control.flushes.lock().unwrap().as_slice(), &[1]);
    f.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn snapshots_require_flush() {
    let f = Fixture::new(config()).await;
    let through = f.storage.write_recovery(&[b"one", b"two"]).await.unwrap();
    f.storage.flush_recovery().await.unwrap();
    let snapshot = RecoverySnapshot {
        required_data: f.storage.cursor_saved(),
        through: through.clone(),
        payload: vec![5],
    };
    f.storage.write_snapshot(&snapshot).await.unwrap();
    f.storage.flush_recovery().await.unwrap();
    assert_eq!(
        f.recovery.control.flushes.lock().unwrap().as_slice(),
        &[2, 2]
    );
    assert_eq!(
        f.storage
            .read_snapshot(NonZeroUsize::new(1024).unwrap())
            .await
            .unwrap(),
        Some(snapshot)
    );
    let after = RecoveryPosition {
        sequence: 0,
        ..through.clone()
    };
    let page = f
        .storage
        .read_recovery(
            &after,
            &through,
            ReadLimits {
                rows: NonZeroUsize::new(1).unwrap(),
                bytes: NonZeroUsize::new(1024).unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(page.next.unwrap().sequence, 1);
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn wrong_receipt_retains() {
    let f = Fixture::new(config()).await;
    f.data.control.wrong_receipt.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    assert!(f.storage.flush().await.is_err());
    assert_eq!(f.storage.status().saved.sequence, 0);
    assert!(f.storage.status().pending_bytes > 0);
    f.data.control.wrong_receipt.store(false, Ordering::SeqCst);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn deadline_retains_admitted() {
    let f = Fixture::new(config()).await;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let error = f.storage.flush().await.unwrap_err();
    assert!(
        matches!(error, Error::Persistence { source } if matches!(source.as_ref(), Error::Backend { kind: BackendFailure::Transient, .. }))
    );
    assert_eq!(f.storage.status().accepted.sequence, 1);
    assert!(f.storage.status().pending_bytes > 0);
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.finish().await;
}

#[tokio::test]
async fn close_seals_admission() {
    let f = Fixture::new(config()).await;
    f.recovery.control.block_read.store(true, Ordering::SeqCst);
    let storage = f.storage.clone();
    let reader = tokio::spawn(async move {
        storage
            .read_snapshot(NonZeroUsize::new(1024).unwrap())
            .await
    });
    f.recovery.control.read_entered.notified().await;

    let storage = f.storage.clone();
    let close = storage.close();
    tokio::pin!(close);
    assert!(
        crate::pending(close.as_mut()).await,
        "read holds the lifecycle lock"
    );
    assert!(f.storage.status().closing);
    assert!(matches!(
        f.storage.write_recovery(&[b"late"]).await,
        Err(Error::Closing { domain: "storage" })
    ));

    f.recovery.control.read_release.notify_one();
    assert!(reader.await.unwrap().unwrap().is_none());
    close.await.unwrap();
    f.finish().await;
}

