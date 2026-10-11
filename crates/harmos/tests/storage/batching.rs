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
use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Fixture, config, record, schema, write};
use harmos::Identifier;
use harmos::schema::{Cardinality, Field, Kind};
use harmos::storage::StreamWrite;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("budget_drains_scratch", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    // Measure the actual admission charge instead of duplicating its formula.
                    let probe = Fixture::new(config()).await;
                    write(&probe.storage, "one").await;
                    let charge = probe.storage.status().pending_bytes;
                    probe.finish().await;

                    let mut policy = config();
                    policy.queue_capacity_bytes = NonZeroUsize::new(charge * 3).unwrap();
                    policy.target_batch_bytes = policy.queue_capacity_bytes;
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    policy.write_batch_entries = NonZeroUsize::new(usize::MAX).unwrap();
                    policy.write_batch_bytes = NonZeroUsize::new(usize::MAX).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    write(&f.storage, "two").await;
                    write(&f.storage, "tri").await;
                    assert_eq!(f.storage.status().pending_bytes, charge * 3);
                    f.storage.flush().await.unwrap();
                    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), vec![1, 1, 1]);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("append_kinds_coalesce", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    f.storage.write_schema(&schema(), &[]).await.unwrap();
                    write(&f.storage, "one").await;
                    write(&f.storage, "two").await;
                    for sequence in 1..=2 {
                        f.storage
                            .write_stream(&[StreamWrite {
                                stream: &Identifier::new("voltage").unwrap(),
                                sequence,
                                record: record(
                                    &schema(),
                                    &Identifier::new(format!("row-{sequence}")).unwrap(),
                                    &[1],
                                ),
                            }])
                            .await
                            .unwrap();
                    }
                    f.storage.write_schema(&schema(), &[]).await.unwrap();
                    write(&f.storage, "three").await;
                    write(&f.storage, "four").await;
                    f.storage.write_recovery(&[b"first"]).await.unwrap();
                    f.storage.write_recovery(&[b"second"]).await.unwrap();
                    f.storage.flush().await.unwrap();
                    f.storage.flush_recovery().await.unwrap();
                    assert_eq!(
                        *f.data.control.write_sizes.lock().unwrap(),
                        vec![1, 2, 2, 1, 2]
                    );
                    assert_eq!(
                        *f.data.control.writes.lock().unwrap(),
                        (1..=8).collect::<Vec<_>>()
                    );
                    assert_eq!(*f.recovery.control.write_sizes.lock().unwrap(), vec![2]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("batch_stops_existing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(config()).await;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    write(&f.storage, "first").await;
    write(&f.storage, "second").await;
    write(&f.storage, "third").await;
    let storage = f.storage.clone();
    let flush = storage.flush();
    tokio::pin!(flush);
    tokio::select! { biased; _ = &mut flush => panic!("write is blocked"), _ = f.data.control.write_entered.notified() => {} }
    write(&f.storage, "fourth").await;
    write(&f.storage, "fifth").await;
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    assert_eq!(flush.await.unwrap().sequence, 3);
    f.storage.flush().await.unwrap();
    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), vec![3, 2]);
    assert_eq!(*f.data.control.flushes.lock().unwrap(), vec![3, 5]);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("pressure_flushes_periodic", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for memory_pressure in [false, true] {
                        let mut policy = config();
                        policy.queue_capacity =
                            NonZeroUsize::new(if memory_pressure { 256 } else { 4 }).unwrap();
                        policy.queue_capacity_bytes =
                            NonZeroUsize::new(if memory_pressure { 8192 } else { 1024 * 1024 })
                                .unwrap();
                        policy.target_batch_bytes = policy.queue_capacity_bytes;
                        policy.max_batch_bytes = policy.queue_capacity_bytes;
                        let queue_limit = policy.queue_capacity.get();
                        let memory_limit = policy.queue_capacity_bytes.get();
                        let f = Fixture::new(policy).await;
                        tokio::time::timeout(Duration::from_secs(5), async {
                            for index in 0..32 {
                                write(&f.storage, &format!("record-{index}")).await;
                                f.storage.write_recovery(&[b"transition"]).await.unwrap();
                                let status = f.storage.status();
                                assert!(status.pending_batches <= queue_limit);
                                assert!(status.pending_bytes <= memory_limit);
                            }
                        })
                        .await
                        .expect("capacity pressure must not wait for the 60-second periodic flush");
                        let status = f.storage.status();
                        assert_eq!(status.accepted.sequence, 32);
                        assert_eq!(status.recovery_accepted.sequence, 32);
                        assert!(status.saved.sequence > 0);
                        assert!(status.recovery_saved.sequence > 0);
                        f.finish().await;
                    }
                });
        });
    }
    {
        scenario_runtime::check("admission_leaves_pressure", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.queue_capacity = NonZeroUsize::new(2).unwrap();
                    let f = Fixture::new(policy).await;
                    f.data.control.block_flush.store(true, Ordering::SeqCst);
                    write(&f.storage, "one").await;
                    write(&f.storage, "two").await;
                    {
                        let blocked = write(&f.storage, "cancelled");
                        tokio::pin!(blocked);
                        tokio::select! {
                            _ = &mut blocked => panic!("the saved receipt is still blocked"),
                            _ = f.data.control.flush_entered.notified() => {}
                        }
                    }
                    assert_eq!(f.storage.status().accepted.sequence, 2);
                    assert_eq!(f.storage.status().saved.sequence, 0);
                    assert_eq!(f.storage.status().pending_batches, 2);
                    f.data.control.block_flush.store(false, Ordering::SeqCst);
                    f.data.control.flush_release.notify_one();
                    tokio::time::timeout(Duration::from_secs(5), async {
                        while f.storage.status().saved.sequence != 2 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    assert_eq!(write(&f.storage, "later").await.sequence, 3);
                    assert_eq!(f.storage.status().saved.sequence, 2);
                    assert_eq!(*f.data.control.flushes.lock().unwrap(), [2]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("pressure_preserves_flush", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut policy = config();
    policy.queue_capacity = NonZeroUsize::new(2).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.block_write.store(true, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let storage = f.storage.clone();
    let flush = storage.flush();
    tokio::pin!(flush);
    tokio::select! { biased; _ = &mut flush => panic!("write is blocked"), _ = f.data.control.write_entered.notified() => {} }
    write(&f.storage, "two").await;
    let blocked = write(&storage, "three");
    tokio::pin!(blocked);
    tokio::select! { biased; _ = &mut blocked => panic!("queue is full"), _ = tokio::task::yield_now() => {} }
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    let (saved, admitted) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(flush, blocked)
    })
    .await
    .unwrap();
    assert_eq!(saved.unwrap().sequence, 1);
    assert_eq!(admitted.sequence, 3);
    assert_eq!(*f.data.control.flushes.lock().unwrap(), [1]);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("failure_retains_work", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.queue_capacity = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    f.data.control.flush_failures.store(1, Ordering::SeqCst);
                    write(&f.storage, "one").await;
                    let key = schema();
                    let item = Identifier::new("not-admitted").unwrap();
                    let inputs = [record(&key, &item, &[1])];
                    let failure = tokio::time::timeout(
                        Duration::from_secs(5),
                        f.storage.write_records(&inputs),
                    )
                    .await
                    .unwrap()
                    .unwrap_err();
                    assert!(matches!(failure, harmos::Error::Persistence { .. }));
                    assert_eq!(f.storage.status().accepted.sequence, 1);
                    assert_eq!(f.storage.status().saved.sequence, 0);
                    assert_eq!(f.storage.status().pending_batches, 1);
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().saved.sequence, 1);
                    assert_eq!(f.storage.status().pending_batches, 0);
                    assert_eq!(*f.data.control.writes.lock().unwrap(), [1, 1]);
                    assert_eq!(f.data.records.lock().unwrap().len(), 1);
                    assert_eq!(*f.data.control.flushes.lock().unwrap(), [1, 1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reservation_never_flush", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.queue_capacity_bytes = NonZeroUsize::new(8192).unwrap();
                    policy.target_batch_bytes = policy.queue_capacity_bytes;
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    let snapshot = tokio::time::timeout(
                        Duration::from_secs(5),
                        f.storage.read_snapshot(NonZeroUsize::new(7000).unwrap()),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert!(snapshot.is_none());
                    assert_eq!(f.storage.status().saved.sequence, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("queued_writes_wait", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    tokio::task::yield_now().await;
                    for item in ["one", "two", "three", "four"] {
                        write(&f.storage, item).await;
                        tokio::task::yield_now().await;
                    }
                    for entry in [b"first".as_slice(), b"second".as_slice()] {
                        f.storage.write_recovery(&[entry]).await.unwrap();
                        tokio::task::yield_now().await;
                    }
                    assert!(f.data.control.writes.lock().unwrap().is_empty());
                    assert!(f.recovery.control.writes.lock().unwrap().is_empty());
                    assert_eq!(f.storage.status().accepted.sequence, 4);
                    assert_eq!(f.storage.status().recovery_accepted.sequence, 2);
                    assert_eq!(f.storage.status().pending_batches, 6);

                    tokio::time::advance(Duration::from_secs(59)).await;
                    tokio::task::yield_now().await;
                    assert!(f.data.control.writes.lock().unwrap().is_empty());
                    assert!(f.recovery.control.writes.lock().unwrap().is_empty());

                    f.data.control.block_flush.store(true, Ordering::SeqCst);
                    f.recovery.control.block_flush.store(true, Ordering::SeqCst);
                    tokio::time::advance(Duration::from_secs(1)).await;
                    f.data.control.flush_entered.notified().await;
                    f.data.control.block_flush.store(false, Ordering::SeqCst);
                    f.data.control.flush_release.notify_one();
                    f.recovery.control.flush_entered.notified().await;
                    f.recovery
                        .control
                        .block_flush
                        .store(false, Ordering::SeqCst);
                    f.recovery.control.flush_release.notify_one();
                    f.storage.flush_recovery().await.unwrap();
                    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [4]);
                    assert_eq!(*f.recovery.control.write_sizes.lock().unwrap(), [2]);
                    assert_eq!(f.storage.status().saved.sequence, 4);
                    assert_eq!(f.storage.status().recovery_saved.sequence, 2);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("barrier_never_write", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    tokio::task::yield_now().await;
                    write(&f.storage, "one").await;
                    write(&f.storage, "two").await;
                    f.data.control.block_flush.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let flush = storage.flush();
                    tokio::pin!(flush);
                    tokio::select! {
                        _ = &mut flush => panic!("the flush is gated"),
                        _ = f.data.control.flush_entered.notified() => {}
                    }
                    write(&f.storage, "later").await;
                    f.data.control.block_flush.store(false, Ordering::SeqCst);
                    f.data.control.flush_release.notify_one();
                    assert_eq!(flush.await.unwrap().sequence, 2);
                    for _ in 0..4 {
                        tokio::task::yield_now().await;
                    }
                    assert_eq!(*f.data.control.writes.lock().unwrap(), [1, 2]);
                    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [2]);
                    assert_eq!(f.storage.status().pending_batches, 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("entry_limit_controls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for (limit, expected) in [(3, vec![3, 3, 2]), (usize::MAX, vec![8])] {
                        let mut policy = config();
                        policy.write_batch_entries = NonZeroUsize::new(limit).unwrap();
                        let f = Fixture::new(policy).await;
                        for index in 0..8 {
                            write(&f.storage, &format!("record-{index}")).await;
                        }
                        f.storage.flush().await.unwrap();
                        assert_eq!(*f.data.control.write_sizes.lock().unwrap(), expected);
                        assert_eq!(
                            *f.data.control.writes.lock().unwrap(),
                            (1..=8).collect::<Vec<_>>()
                        );
                        assert_eq!(f.storage.status().pending_bytes, 0);
                        f.finish().await;
                    }
                });
        });
    }
    {
        scenario_runtime::check("scratch_limit_controls", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for (bytes, expected) in [(1, vec![1, 1, 1]), (64 * 1024, vec![3])] {
                        let mut policy = config();
                        policy.write_batch_bytes = NonZeroUsize::new(bytes).unwrap();
                        let f = Fixture::new(policy).await;
                        let key = schema();
                        let payload = vec![7; 1024];
                        for index in 0..3 {
                            let item = Identifier::new(format!("record-{index}")).unwrap();
                            f.storage
                                .write_records(&[record(&key, &item, &payload)])
                                .await
                                .unwrap();
                        }
                        let admitted = f.storage.status().pending_bytes;
                        f.data.control.block_write.store(true, Ordering::SeqCst);
                        let storage = f.storage.clone();
                        let flush = storage.flush();
                        tokio::pin!(flush);
                        tokio::select! {
                            _ = &mut flush => panic!("write is blocked"),
                            _ = f.data.control.write_entered.notified() => {}
                        }
                        assert!(f.storage.status().pending_bytes - admitted <= bytes);
                        f.data.control.block_write.store(false, Ordering::SeqCst);
                        f.data.control.write_release.notify_one();
                        flush.await.unwrap();
                        assert_eq!(*f.data.control.write_sizes.lock().unwrap(), expected);
                        assert_eq!(f.storage.status().pending_bytes, 0);
                        f.finish().await;
                    }
                });
        });
    }
    {
        scenario_runtime::check("limits_never_split", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.write_batch_entries = NonZeroUsize::new(1).unwrap();
                    policy.write_batch_bytes = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    let key = schema();
                    let item = Identifier::new("reading").unwrap();
                    f.storage
                        .write_records(&[record(&key, &item, &[1]), record(&key, &item, &[2])])
                        .await
                        .unwrap();
                    write(&f.storage, "later").await;
                    f.storage.flush().await.unwrap();
                    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [2, 1]);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("progress_preserves_cursor", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "before").await;
                    let snapshot = harmos::storage::recovery::RecoverySnapshot {
                        through: f.storage.status().recovery_accepted,
                        payload: vec![7],
                    };
                    f.storage.write_snapshot(&snapshot).await.unwrap();
                    write(&f.storage, "after").await;
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().saved.sequence, 2);
                    assert_eq!(f.storage.status().pending_batches, 1);
                    assert!(f.recovery.snapshot.lock().unwrap().is_none());
                    f.storage.flush_recovery().await.unwrap();
                    assert_eq!(*f.recovery.snapshot.lock().unwrap(), Some(snapshot));
                    assert_eq!(*f.recovery.control.flushes.lock().unwrap(), [0]);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("target_bytes_trigger", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Fixture::new(config()).await;
                    write(&probe.storage, "one").await;
                    let charge = probe.storage.status().pending_bytes;
                    probe.finish().await;

                    let mut policy = config();
                    policy.queue_capacity_bytes = NonZeroUsize::new(charge * 16).unwrap();
                    policy.target_batch_bytes = NonZeroUsize::new(charge * 2).unwrap();
                    policy.max_batch_bytes = policy.target_batch_bytes;
                    let f = Fixture::new(policy).await;
                    f.data.control.block_write.store(true, Ordering::SeqCst);
                    write(&f.storage, "one").await;
                    tokio::task::yield_now().await;
                    assert!(f.data.control.writes.lock().unwrap().is_empty());
                    write(&f.storage, "two").await;
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        f.data.control.write_entered.notified(),
                    )
                    .await
                    .unwrap();
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    assert_eq!(f.storage.status().pending_batches, 2);
                    f.data.control.block_write.store(false, Ordering::SeqCst);
                    f.data.control.write_release.notify_one();
                    tokio::time::timeout(Duration::from_secs(5), async {
                        while f.storage.cursor_saved().sequence != 2 {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    write(&f.storage, "tri").await;
                    tokio::task::yield_now().await;
                    assert_eq!(*f.data.control.writes.lock().unwrap(), [1, 2]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("splits_groups_cancelled", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_batch_bytes = NonZeroUsize::new(1024).unwrap();
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let f = Fixture::new(policy).await;
                    let key = schema();
                    let item = Identifier::new("group").unwrap();
                    let payload = vec![7; 100];
                    let records: Vec<_> = (0..8).map(|_| record(&key, &item, &payload)).collect();
                    f.data.control.block_write.store(true, Ordering::SeqCst);
                    let accepted = f.storage.write_records(&records).await.unwrap();
                    let admitted = f.storage.status().pending_bytes;
                    {
                        let flush = f.storage.flush_until(&accepted);
                        tokio::pin!(flush);
                        tokio::select! {
                            _ = &mut flush => panic!("physical write is blocked"),
                            _ = f.data.control.write_entered.notified() => {}
                        }
                        f.data.control.write_release.notify_one();
                        tokio::select! {
                            _ = &mut flush => panic!("the complete admitted group is not durable"),
                            _ = f.data.control.write_entered.notified() => {}
                        }
                        let saved = f.storage.cursor_saved().sequence;
                        assert!(saved > 0 && saved < accepted.sequence);
                        assert_eq!(f.storage.status().pending_batches, 1);
                        assert_eq!(f.storage.status().pending_bytes, admitted);
                    }
                    f.data.control.block_write.store(false, Ordering::SeqCst);
                    f.data.control.write_release.notify_one();
                    tokio::time::timeout(Duration::from_secs(5), async {
                        while f.storage.cursor_saved() != accepted {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert_eq!(
                        *f.data.control.writes.lock().unwrap(),
                        (1..=8).collect::<Vec<_>>()
                    );
                    let sizes = f.data.control.write_bytes.lock().unwrap().clone();
                    assert!(sizes.len() > 1 && sizes.iter().all(|bytes| *bytes <= 1024));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("item_refuses_whole", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_batch_bytes = NonZeroUsize::new(512).unwrap();
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let f = Fixture::new(policy).await;
                    let key = schema();
                    let item = Identifier::new("oversized").unwrap();
                    let large = vec![255; 512];
                    let inputs = [record(&key, &item, b"small"), record(&key, &item, &large)];
                    assert!(matches!(
                        f.storage.write_records(&inputs).await,
                        Err(harmos::Error::InvalidInput {
                            field: "storage admission",
                            ..
                        })
                    ));
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert!(f.data.control.writes.lock().unwrap().is_empty());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("suffix_retries_rewriting", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_batch_bytes = NonZeroUsize::new(1024).unwrap();
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let f = Fixture::new(policy).await;
                    let key = schema();
                    let item = Identifier::new("group").unwrap();
                    let payload = vec![7; 100];
                    let records: Vec<_> = (0..8).map(|_| record(&key, &item, &payload)).collect();
                    f.data.control.block_write.store(true, Ordering::SeqCst);
                    let accepted = f.storage.write_records(&records).await.unwrap();
                    let saved_prefix;
                    {
                        let flush = f.storage.flush_until(&accepted);
                        tokio::pin!(flush);
                        tokio::select! {
                            _ = &mut flush => panic!("first physical batch is blocked"),
                            _ = f.data.control.write_entered.notified() => {}
                        }
                        f.data.control.write_release.notify_one();
                        tokio::select! {
                            _ = &mut flush => panic!("second physical batch is blocked"),
                            _ = f.data.control.write_entered.notified() => {}
                        }
                        saved_prefix = f.storage.cursor_saved();
                        assert!(
                            saved_prefix.sequence > 0 && saved_prefix.sequence < accepted.sequence
                        );
                        f.data.control.failures.store(1, Ordering::SeqCst);
                        f.data.control.block_write.store(false, Ordering::SeqCst);
                        f.data.control.write_release.notify_one();
                        assert!(flush.await.is_err());
                    }
                    assert_eq!(f.storage.cursor_saved(), saved_prefix);
                    assert_eq!(f.storage.status().pending_batches, 1);
                    assert_eq!(f.storage.flush_until(&accepted).await.unwrap(), accepted);
                    let writes = f.data.control.writes.lock().unwrap().clone();
                    for position in 1..=saved_prefix.sequence {
                        assert_eq!(
                            writes
                                .iter()
                                .filter(|written| **written == position)
                                .count(),
                            1
                        );
                    }
                    assert_eq!(f.data.records.lock().unwrap().len(), 8);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("admission_keeps_memory", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fields: Vec<_> = (1..=64)
                        .map(|tag| Field {
                            name: Cow::Owned(format!("field-{tag}")),
                            tag,
                            kind: Kind::U64,
                            cardinality: Cardinality::Required,
                        })
                        .collect();
                    let mut policy = config();
                    policy.queue_capacity_bytes = NonZeroUsize::new(16 * 1024).unwrap();
                    policy.target_batch_bytes = policy.queue_capacity_bytes;
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    let f = Fixture::new(policy.clone()).await;
                    let key = schema();
                    let position = f.storage.write_schema(&key, &fields).await.unwrap();
                    assert_eq!(position.sequence, 1);
                    assert!(f.storage.status().pending_bytes < policy.queue_capacity_bytes.get());
                    f.storage.flush().await.unwrap();
                    {
                        let saved = f.data.schemas.lock().unwrap();
                        assert_eq!(saved.len(), 1);
                        assert_eq!(saved[&1].fields, fields);
                    }
                    f.finish().await;

                    policy.max_batch_bytes = NonZeroUsize::new(1024).unwrap();
                    policy.target_batch_bytes = policy.max_batch_bytes;
                    let f = Fixture::new(policy).await;
                    assert!(matches!(
                        f.storage.write_schema(&key, &fields).await,
                        Err(harmos::Error::InvalidInput {
                            field: "storage admission",
                            reason: "one entry exceeds max_batch_bytes",
                        })
                    ));
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert!(f.data.schemas.lock().unwrap().is_empty());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("schema_validation_reserves", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    static CHILDREN: std::sync::LazyLock<Vec<Field>> =
                        std::sync::LazyLock::new(|| {
                            (1..=256)
                                .map(|tag| Field {
                                    name: Cow::Owned(format!("child-{tag}")),
                                    tag,
                                    kind: Kind::U64,
                                    cardinality: Cardinality::Required,
                                })
                                .collect()
                        });
                    let fields = [Field {
                        name: Cow::Borrowed("parent"),
                        tag: 1,
                        kind: Kind::Message {
                            fields: Cow::Borrowed(CHILDREN.as_slice()),
                        },
                        cardinality: Cardinality::Required,
                    }];
                    let mut policy = config();
                    policy.queue_capacity_bytes = NonZeroUsize::new(4096).unwrap();
                    policy.target_batch_bytes = policy.queue_capacity_bytes;
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    let f = Fixture::new(policy.clone()).await;
                    assert!(matches!(
                        f.storage.write_schema(&schema(), &fields).await,
                        Err(harmos::Error::InvalidInput {
                            field: "storage schema",
                            reason: "validation scratch exceeds write memory budget"
                        })
                    ));
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert!(f.data.schemas.lock().unwrap().is_empty());
                    f.finish().await;
                    policy.queue_capacity_bytes = NonZeroUsize::new(64 * 1024).unwrap();
                    policy.target_batch_bytes = policy.queue_capacity_bytes;
                    policy.max_batch_bytes = policy.queue_capacity_bytes;
                    let f = Fixture::new(policy).await;
                    f.storage.write_schema(&schema(), &fields).await.unwrap();
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
}

/* === Tests === */
