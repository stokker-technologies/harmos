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
use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{Fixture, config, record, schema, write};
use harmos::Identifier;
use harmos::schema::Cardinality;
use harmos::schema::graph::{Field, Kind};
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

#[tokio::test]
async fn append_kinds_coalesce() {
    let f = Fixture::new(config()).await;
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
}

#[tokio::test]
async fn batch_stops_existing() {
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
}

#[tokio::test]
async fn admission_leaves_pressure() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
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
}

#[tokio::test]
async fn pressure_preserves_flush() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(2).unwrap();
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
}

#[tokio::test(start_paused = true)]
async fn failure_retains_work() {
    let mut policy = config();
    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
    let f = Fixture::new(policy).await;
    f.data.control.flush_failures.store(1, Ordering::SeqCst);
    write(&f.storage, "one").await;
    let key = schema();
    let item = Identifier::new("not-admitted").unwrap();
    let inputs = [record(&key, &item, &[1])];
    let failure = tokio::time::timeout(Duration::from_secs(5), f.storage.write_records(&inputs))
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
}

#[tokio::test(start_paused = true)]
async fn reservation_never_flush() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(8192).unwrap();
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
}

#[tokio::test(start_paused = true)]
async fn queued_writes_wait() {
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

    // The Private Periodic Save Interval Is 100 Milliseconds.
    tokio::time::advance(Duration::from_millis(99)).await;
    tokio::task::yield_now().await;
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    assert!(f.recovery.control.writes.lock().unwrap().is_empty());

    f.data.control.block_flush.store(true, Ordering::SeqCst);
    f.recovery.control.block_flush.store(true, Ordering::SeqCst);
    tokio::time::advance(Duration::from_millis(1)).await;
    // Independent State Work May Reach Its Flush Before Data.
    tokio::join!(
        async {
            f.data.control.flush_entered.notified().await;
            f.data.control.block_flush.store(false, Ordering::SeqCst);
            f.data.control.flush_release.notify_one();
        },
        async {
            f.recovery.control.flush_entered.notified().await;
            f.recovery
                .control
                .block_flush
                .store(false, Ordering::SeqCst);
            f.recovery.control.flush_release.notify_one();
        }
    );
    f.storage.flush_recovery().await.unwrap();
    f.storage.flush().await.unwrap();
    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [4]);
    assert_eq!(*f.recovery.control.write_sizes.lock().unwrap(), [2]);
    assert_eq!(f.storage.status().saved.sequence, 4);
    assert_eq!(f.storage.status().recovery_saved.sequence, 2);
    f.finish().await;
}

#[tokio::test(start_paused = true)]
async fn barrier_never_write() {
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
}

#[tokio::test]
async fn progress_preserves_cursor() {
    let f = Fixture::new(config()).await;
    write(&f.storage, "before").await;
    let snapshot = harmos::storage::state::RecoverySnapshot {
        required_data: f.storage.cursor_saved(),
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
}

#[tokio::test]
async fn atomic_batch_settles() {
    let f = Fixture::new(config()).await;
    let key = schema();
    let item = Identifier::new("group").unwrap();
    let payload = vec![7; 100];
    let records: Vec<_> = (0..8).map(|_| record(&key, &item, &payload)).collect();
    f.data.control.block_write.store(true, Ordering::SeqCst);
    let accepted = f.storage.write_records(&records).await.unwrap();
    let admitted = f.storage.status().pending_bytes;
    let flushing = {
        let storage = f.storage.clone();
        tokio::spawn(async move { storage.flush().await })
    };
    f.data.control.write_entered.notified().await;
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    assert_eq!(f.storage.status().pending_bytes, admitted);
    f.data.control.block_write.store(false, Ordering::SeqCst);
    f.data.control.write_release.notify_one();
    assert_eq!(flushing.await.unwrap().unwrap(), accepted);
    assert_eq!(*f.data.control.write_sizes.lock().unwrap(), [8]);
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn item_refuses_whole() {
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(4096).unwrap();
    let f = Fixture::new(policy).await;
    let key = schema();
    let item = Identifier::new("oversized").unwrap();
    let large = vec![255; 8192];
    let inputs = [record(&key, &item, b"small"), record(&key, &item, &large)];
    assert!(matches!(
        f.storage.write_records(&inputs).await,
        Err(harmos::Error::ResourceExhausted { .. })
    ));
    assert_eq!(f.storage.cursor_accepted().sequence, 0);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert!(f.data.control.writes.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn whole_batch_retries() {
    let f = Fixture::new(config()).await;
    let key = schema();
    let item = Identifier::new("group").unwrap();
    let payload = vec![7; 100];
    let records: Vec<_> = (0..8).map(|_| record(&key, &item, &payload)).collect();
    let accepted = f.storage.write_records(&records).await.unwrap();
    f.data.control.failures.store(1, Ordering::SeqCst);
    assert!(f.storage.flush_until(&accepted).await.is_err());
    assert_eq!(f.storage.cursor_saved().sequence, 0);
    assert_eq!(f.storage.status().pending_batches, 1);
    assert_eq!(f.storage.flush_until(&accepted).await.unwrap(), accepted);
    assert_eq!(f.data.records.lock().unwrap().len(), 8);
    assert!(
        f.data
            .control
            .write_sizes
            .lock()
            .unwrap()
            .iter()
            .all(|size| *size == 8)
    );
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

#[tokio::test]
async fn admission_keeps_memory() {
    let fields: Vec<_> = (1..=64)
        .map(|tag| Field {
            name: Cow::Owned(format!("field-{tag}")),
            tag,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        })
        .collect();
    let graph = harmos::schema::graph::Schema {
        root: 0,
        definitions: vec![fields],
    };
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(16 * 1024).unwrap();
    let f = Fixture::new(policy.clone()).await;
    let key = schema();
    let position = f.storage.write_schema(&key, &graph).await.unwrap();
    assert_eq!(position.sequence, 1);
    assert!(f.storage.status().pending_bytes < policy.request.max_pending_write_bytes.get());
    f.storage.flush().await.unwrap();
    {
        let saved = f.data.schemas.lock().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[&1].schema, graph);
        for definitions in [1usize, 32, 256] {
            let mut metadata = saved[&1].clone();
            let fields = (1..definitions)
                .map(|definition| Field {
                    name: Cow::Owned(format!("child_{definition}")),
                    tag: definition as u32,
                    cardinality: Cardinality::Repeated,
                    kind: Kind::Message { definition },
                })
                .collect();
            metadata.schema.definitions = vec![Vec::new(); definitions];
            metadata.schema.definitions[0] = fields;
            let encoded = serde_json::to_vec(&metadata).unwrap();
            let admitted =
                harmos::storage::StorageSchema::decoding_memory_bytes(encoded.len()).unwrap();
            let decoded: harmos::storage::StorageSchema = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(decoded, metadata);
            assert!(
                harmos::MemorySize::memory_size(&decoded)
                    <= admitted + std::mem::size_of::<harmos::storage::StorageSchema>()
            );
        }
        assert!(harmos::storage::StorageSchema::decoding_memory_bytes(usize::MAX).is_none());
        assert!(harmos::storage::StorageSchema::decoding_memory_bytes(usize::MAX / 36).is_none());
    }
    f.finish().await;

    policy.request.max_pending_write_bytes = NonZeroUsize::new(1024).unwrap();
    let f = Fixture::new(policy).await;
    assert!(matches!(
        f.storage.write_schema(&key, &graph).await,
        Err(harmos::Error::ResourceExhausted { .. }
            | harmos::Error::InvalidInput {
                field: "storage admission",
                ..
            })
    ));
    assert_eq!(f.storage.cursor_accepted().sequence, 0);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert!(f.data.schemas.lock().unwrap().is_empty());
    f.finish().await;
}

#[tokio::test]
async fn schema_validation_reserves() {
    static CHILDREN: std::sync::LazyLock<Vec<Field>> = std::sync::LazyLock::new(|| {
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
        kind: Kind::Message { definition: 1 },
        cardinality: Cardinality::Required,
    }];
    let graph = harmos::schema::graph::Schema {
        root: 0,
        definitions: vec![fields.to_vec(), CHILDREN.to_vec()],
    };
    let mut policy = config();
    policy.request.max_pending_write_bytes = NonZeroUsize::new(4096).unwrap();
    let f = Fixture::new(policy.clone()).await;
    assert!(matches!(
        f.storage.write_schema(&schema(), &graph).await,
        Err(harmos::Error::ResourceExhausted {
            resource: "storage write memory"
        })
    ));
    assert_eq!(f.storage.cursor_accepted().sequence, 0);
    assert_eq!(f.storage.status().pending_bytes, 0);
    assert!(f.data.schemas.lock().unwrap().is_empty());
    f.finish().await;
    policy.request.max_pending_write_bytes = NonZeroUsize::new(64 * 1024).unwrap();
    let f = Fixture::new(policy).await;
    f.storage.write_schema(&schema(), &graph).await.unwrap();
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().pending_bytes, 0);
    f.finish().await;
}

/* === Tests === */
