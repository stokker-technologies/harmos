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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn limits(rows: usize, bytes: usize) -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(rows).unwrap(),
        bytes: NonZeroUsize::new(bytes).unwrap(),
    }
}

async fn positions(mut reader: RecordQueryResult<Vec<u8>>) -> Vec<u64> {
    let mut result = Vec::new();
    while let Some(page) = reader.next_page().await.unwrap() {
        result.extend(page.iter().map(|row| row.position.sequence));
    }
    result
}

pub(super) async fn wait_release(f: &Fixture, count: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while f.data.control.releases.load(Ordering::SeqCst) < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("order_survives_compaction", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "durable").await;
                    write(&f.storage, "second").await;
                    f.storage.flush().await.unwrap();
                    write(&f.storage, "buffered").await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    assert_eq!(view.accepted().unwrap().sequence, 3);
                    assert_eq!(view.saved().unwrap().sequence, 2);
                    f.storage.flush().await.unwrap();
                    write(&f.storage, "later").await;
                    f.storage.flush().await.unwrap();
                    // The backend keeps pinned versions after current physical tables compact.
                    f.data.records.lock().unwrap().clear();

                    for order in [ReadOrder::Ascending, ReadOrder::Descending] {
                        let mut query = record_query(limits(1, 2048));
                        query.options.order = order;
                        let expected = match order {
                            ReadOrder::Ascending => [1, 2],
                            ReadOrder::Descending => [2, 1],
                        };
                        assert_eq!(
                            positions(f.storage.query_in(&view, query).unwrap()).await,
                            expected
                        );
                    }
                    assert!(f.storage.status().read_bytes > 0);
                    drop(view);
                    wait_release(&f, 1).await;
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reader_pages_under", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    for id in ["one", "two"] {
                        write(&f.storage, id).await;
                    }
                    f.storage.flush().await.unwrap();
                    for id in ["three", "four"] {
                        write(&f.storage, id).await;
                    }
                    f.storage.flush().await.unwrap();
                    let mut reader = f.storage.query(record_query(limits(8, 512))).await.unwrap();
                    let mut seen = Vec::new();
                    while let Some(page) = reader.next_page().await.unwrap() {
                        assert!(
                            page.iter()
                                .map(|row| serde_json::to_vec(row).unwrap().len())
                                .sum::<usize>()
                                <= 512
                        );
                        seen.extend(page.iter().map(|row| row.position.sequence));
                    }
                    assert_eq!(seen, [1, 2, 3, 4]);
                    wait_release(&f, 1).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("collections_share_frontier", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let key = schema();
                    let item = Identifier::new("item").unwrap();
                    let scope = Identifier::new("selected").unwrap();
                    let stream = Identifier::new("voltage").unwrap();
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
                    let mut input = record(&key, &item, &[1]);
                    input.scope = Some(&scope);
                    f.storage.write_records(&[input]).await.unwrap();
                    f.storage.flush().await.unwrap();
                    f.storage
                        .write_stream(&[StreamWrite {
                            stream: &stream,
                            sequence: 72,
                            record: input,
                        }])
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    write(&f.storage, "excluded").await;
                    let mut records = record_query(limits(1, 2048));
                    records.schema = key.clone();
                    records.scope = Some(scope.clone());
                    records.item = Some(item);
                    assert_eq!(
                        positions(f.storage.query_in(&view, records).unwrap()).await,
                        [2]
                    );
                    let mut rows = stream_query(limits(8, 4096));
                    rows.stream = Some(stream);
                    rows.scope = Some(scope);
                    let mut reader = f.storage.query_in(&view, rows).unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(page.len(), 1);
                    assert_eq!(page[0].sequence, 72);
                    let mut schemas = schema_query(limits(8, 4096));
                    schemas.schema = Some(key);
                    let page = f
                        .storage
                        .query_in(&view, schemas)
                        .unwrap()
                        .next_page()
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(page[0].position.sequence, 1);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reads_never_clear", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    f.data.control.permanent.store(true, Ordering::SeqCst);
                    write(&f.storage, "pending").await;
                    assert!(f.storage.flush().await.is_err());
                    let attempts = f.data.control.writes.lock().unwrap().len();
                    assert_eq!(
                        positions(
                            f.storage
                                .query(record_query(limits(1, 2048)))
                                .await
                                .unwrap()
                        )
                        .await,
                        Vec::<u64>::new()
                    );
                    assert!(f.storage.status().failure.is_some());
                    // The expired first view must not block a second acquisition behind cleanup.
                    assert_eq!(
                        positions(
                            f.storage
                                .query(record_query(limits(1, 2048)))
                                .await
                                .unwrap()
                        )
                        .await,
                        Vec::<u64>::new()
                    );
                    assert_eq!(f.data.control.writes.lock().unwrap().len(), attempts);
                    f.data.control.permanent.store(false, Ordering::SeqCst);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancelled_view_acquisition", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let task = tokio::spawn(async move {
                        storage.view_with_limits(config().query_execution).await
                    });
                    f.data.control.view_entered.notified().await;
                    task.abort();
                    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    wait_release(&f, 1).await;
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_preserves_cursor", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "one").await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 1024)))
                        .await
                        .unwrap();
                    let baseline = f.storage.status().read_bytes;
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    assert!(
                        tokio::time::timeout(Duration::from_millis(10), reader.next_page())
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    assert_eq!(positions(reader).await, [1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("expiry_never_rebases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 1024)))
                        .await
                        .unwrap();
                    tokio::time::advance(Duration::from_millis(11)).await;
                    assert!(matches!(reader.next_page().await, Err(Error::ViewExpired)));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("pressure_refuses_views", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_views = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let old = f.storage.view_with_limits(f.execution).await.unwrap();
                    assert!(matches!(
                        f.storage.view_with_limits(f.execution).await,
                        Err(Error::ResourceExhausted {
                            resource: "storage views"
                        })
                    ));
                    write(&f.storage, "later").await;
                    f.storage.flush().await.unwrap();
                    assert_eq!(
                        positions(
                            f.storage
                                .query_in(&old, record_query(limits(1, 2048)))
                                .unwrap()
                        )
                        .await,
                        [1]
                    );
                    drop(old);
                    wait_release(&f, 1).await;
                    assert_eq!(
                        positions(
                            f.storage
                                .query(record_query(limits(1, 2048)))
                                .await
                                .unwrap()
                        )
                        .await,
                        [1, 2]
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("pressure_preserves_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.request.max_pending_write_bytes = NonZeroUsize::new(50_000).unwrap();
                    policy.target_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.max_batch_bytes = policy.request.max_pending_write_bytes;
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "held").await;
                    let old = f.storage.view_with_limits(f.execution).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let payload = vec![0; 16_384];
                    f.storage
                        .write_records(&[record(
                            &schema(),
                            &Identifier::new("large").unwrap(),
                            &payload,
                        )])
                        .await
                        .unwrap();
                    assert!(f.storage.status().pending_bytes <= 50_000);
                    assert!(
                        positions(
                            f.storage
                                .query_in(&old, record_query(limits(1, 2048)))
                                .unwrap()
                        )
                        .await
                        .is_empty()
                    );
                    assert_eq!(old.saved().unwrap().sequence, 0);
                    drop(old);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("snapshot_release_retryable", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    f.data.control.fail_release.store(true, Ordering::SeqCst);
                    assert!(f.storage.close().await.is_err());
                    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 0);
                    assert!(f.storage.status().closing);
                    f.data.control.fail_release.store(false, Ordering::SeqCst);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("results_never_advance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "requested").await;
                    f.storage.flush().await.unwrap();
                    let mut query = record_query(limits(1, 2048));
                    query.item = Some(Identifier::new("requested").unwrap());
                    let mut reader = f.storage.query(query).await.unwrap();
                    f.data.control.invalid_page.store(true, Ordering::SeqCst);
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::InvalidInput { .. })
                    ));
                    f.data.control.invalid_page.store(false, Ordering::SeqCst);
                    assert_eq!(positions(reader).await, [1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("row_refuses_small", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "one").await;
                    f.storage.flush().await.unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let mut reader = f
                        .storage
                        .query_in(&view, record_query(limits(1, 1)))
                        .unwrap();
                    for _ in 0..2 {
                        assert!(matches!(
                            reader.next_page().await,
                            Err(Error::InvalidInput { .. })
                        ));
                    }
                    drop(reader);
                    assert_eq!(
                        positions(
                            f.storage
                                .query_in(&view, record_query(limits(1, 2048)))
                                .unwrap()
                        )
                        .await,
                        [1]
                    );
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cleanup_outlives_operation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut policy = config();
    policy.io_timeout = Duration::from_millis(20);
    let f = Fixture::new(policy).await;
    let view = f.storage.view_with_limits(f.execution).await.unwrap();
    f.data.control.block_release.store(true, Ordering::SeqCst);
    drop(view);
    f.data.control.release_entered.notified().await;
    let storage = f.storage.clone();
    let closing = storage.close();
    tokio::pin!(closing);
    tokio::select! {
        result = &mut closing => panic!("cleanup completed before release: {result:?}"),
        _ = tokio::time::sleep(Duration::from_millis(60)) => {},
    }
    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 1);
    assert_eq!(f.data.control.closes.load(Ordering::SeqCst), 0);
    f.data.control.block_release.store(false, Ordering::SeqCst);
    f.data.control.release_release.notify_one();
    closing.await.unwrap();
    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 1);
    assert_eq!(f.storage.status().read_bytes, 0);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("shutdown_waits_inflight", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_views = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let reading = tokio::spawn(async move { reader.next_page().await });
                    f.data.control.read_entered.notified().await;
                    let storage = f.storage.clone();
                    let replacement = storage.close();
                    tokio::pin!(replacement);
                    tokio::select! {
                        biased;
                        _ = &mut replacement => panic!("read still owns its backend pin"),
                        _ = tokio::task::yield_now() => {}
                    }
                    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 0);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    f.data.control.read_release.notify_one();
                    assert!(matches!(reading.await.unwrap(), Err(Error::ViewExpired)));
                    replacement.await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("views_never_starve", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let first = tokio::spawn(async move {
                        storage.view_with_limits(config().query_execution).await
                    });
                    f.data.control.view_entered.notified().await;

                    let storage = f.storage.clone();
                    let second = storage.view_with_limits(config().query_execution);
                    tokio::pin!(second);
                    tokio::select! {
                        biased;
                        _ = &mut second => panic!("first backend acquisition is blocked"),
                        _ = tokio::task::yield_now() => {}
                    }
                    write(&f.storage, "must-progress").await;
                    let flushing = f.storage.clone();
                    let flushing = flushing.flush();
                    tokio::pin!(flushing);
                    tokio::select! {
                        biased;
                        _ = &mut flushing => panic!("first backend acquisition is blocked"),
                        _ = tokio::task::yield_now() => {}
                    }
                    f.data.control.view_release.notify_one();
                    let first = first.await.unwrap().unwrap();
                    f.data.control.view_entered.notified().await;
                    assert_eq!(f.data.control.writes.lock().unwrap().as_slice(), &[1]);

                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    drop(second.await.unwrap());
                    flushing.await.unwrap();
                    drop(first);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("alone_never_expire", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "first").await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(5), write(&f.storage, "later"))
                        .await
                        .expect("queue pressure flushes without expiring the view");
                    assert_eq!(f.storage.status().saved.sequence, 1);
                    assert_eq!(
                        positions(
                            f.storage
                                .query_in(&view, record_query(limits(1, 2048)))
                                .unwrap()
                        )
                        .await,
                        Vec::<u64>::new()
                    );
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("large_cursor_metadata", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.request.max_pending_write_bytes = NonZeroUsize::new(200_000).unwrap();
                    policy.target_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.max_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.budget.read_memory_bytes = NonZeroUsize::new(200_000).unwrap();
                    policy.query_execution.memory_bytes = NonZeroUsize::new(64_000).unwrap();
                    policy.query_decoded_bytes = policy.budget.read_memory_bytes;
                    let f = Fixture::new(policy).await;
                    let item = Identifier::new("x".repeat(32_768)).unwrap();
                    let key = schema();
                    f.storage
                        .write_records(&[record(&key, &item, &[1])])
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    f.storage
                        .write_records(&[record(&key, &item, &[2])])
                        .await
                        .unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    f.storage.flush().await.unwrap();
                    let mut query = record_query(limits(1, 64_000));
                    query.options.decoded_bytes = NonZeroUsize::new(40_000);
                    query.item = Some(item);
                    let mut reader = f.storage.query_in(&view, query).unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert!(f.storage.status().read_bytes <= 200_000);
                    drop(reader);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    // The Frozen View Still Owns The Same Saved Row After Metadata Refusal.
                    let mut query = record_query(limits(1, 64_000));
                    query.options.decoded_bytes = NonZeroUsize::new(40_000);
                    let mut reader = f.storage.query_in(&view, query).unwrap();
                    assert_eq!(
                        reader.next_page().await.unwrap().unwrap()[0]
                            .position
                            .sequence,
                        1
                    );
                    drop(reader);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("view_never_consume", || {
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
                    policy.request.max_pending_write_bytes = NonZeroUsize::new(charge + 1).unwrap();
                    policy.target_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.max_batch_bytes = policy.request.max_pending_write_bytes;
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    assert_eq!(view.accepted().unwrap().sequence, 1);
                    assert_eq!(view.saved().unwrap().sequence, 0);
                    assert_eq!(f.storage.status().pending_bytes, charge);
                    assert!(f.storage.status().read_bytes > 0);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("active_pages_renew", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(policy).await;
                    for id in ["one", "two", "three", "four"] {
                        write(&f.storage, id).await;
                    }
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    write(&f.storage, "later").await;
                    f.storage.flush().await.unwrap();

                    for expected in 1..=4 {
                        tokio::time::advance(Duration::from_millis(6)).await;
                        let page = reader.next_page().await.unwrap().unwrap();
                        assert_eq!(page.len(), 1);
                        assert_eq!(page[0].position.sequence, expected);
                    }
                    assert!(reader.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("time_not_reader", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(policy).await;
                    for id in ["one", "two", "three"] {
                        write(&f.storage, id).await;
                    }
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let page = reader.next_page();
                        tokio::pin!(page);
                        tokio::select! {
                            _ = &mut page => panic!("backend read is gated"),
                            _ = f.data.control.read_entered.notified() => {}
                        }
                        tokio::time::advance(Duration::from_millis(20)).await;
                        f.data.control.block_read.store(false, Ordering::SeqCst);
                        f.data.control.read_release.notify_one();
                        assert_eq!(page.await.unwrap().unwrap()[0].position.sequence, 1);
                    }
                    tokio::time::advance(Duration::from_millis(9)).await;
                    assert_eq!(
                        reader.next_page().await.unwrap().unwrap()[0]
                            .position
                            .sequence,
                        2
                    );
                    tokio::time::advance(Duration::from_millis(11)).await;
                    assert!(matches!(reader.next_page().await, Err(Error::ViewExpired)));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("snapshot_acquisition_precedes", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(policy).await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let view = storage.view_with_limits(config().query_execution);
                    tokio::pin!(view);
                    tokio::select! {
                        _ = &mut view => panic!("snapshot acquisition is gated"),
                        _ = f.data.control.view_entered.notified() => {}
                    }
                    tokio::time::advance(Duration::from_millis(20)).await;
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    let view = view.await.unwrap();
                    tokio::time::advance(Duration::from_millis(9)).await;
                    assert_eq!(view.saved().unwrap().sequence, 0);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("queued_reads_suspend", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for cancel in [false, true] {
                        let mut policy = config();
                        policy.view_idle_timeout = Duration::from_millis(10);
                        let f = Fixture::new(policy).await;
                        write(&f.storage, "one").await;
                        f.storage.flush().await.unwrap();
                        let mut first = f
                            .storage
                            .query(record_query(limits(1, 2048)))
                            .await
                            .unwrap();
                        let mut second = f
                            .storage
                            .query(record_query(limits(1, 2048)))
                            .await
                            .unwrap();
                        f.data.control.block_read.store(true, Ordering::SeqCst);
                        let first = tokio::spawn(async move { first.next_page().await });
                        f.data.control.read_entered.notified().await;
                        {
                            let page = second.next_page();
                            tokio::pin!(page);
                            tokio::select! {
                                biased;
                                _ = &mut page => panic!("first reader owns the I/O lock"),
                                _ = tokio::task::yield_now() => {}
                            }
                            tokio::time::advance(Duration::from_millis(20)).await;
                            if !cancel {
                                f.data.control.block_read.store(false, Ordering::SeqCst);
                                f.data.control.read_release.notify_one();
                                assert_eq!(page.await.unwrap().unwrap()[0].position.sequence, 1);
                            }
                        }
                        if cancel {
                            tokio::time::advance(Duration::from_millis(11)).await;
                            f.data.control.block_read.store(false, Ordering::SeqCst);
                            f.data.control.read_release.notify_one();
                            assert!(matches!(second.next_page().await, Err(Error::ViewExpired)));
                        }
                        assert!(first.await.unwrap().is_ok());
                        f.finish().await;
                    }
                });
        });
    }
    {
        scenario_runtime::check("queries_pin_saved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    write(&f.storage, "pending").await;
                    let reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    assert_eq!(positions(reader).await, [1]);
                    assert_eq!(
                        positions(
                            f.storage
                                .query(record_query(limits(1, 2048)))
                                .await
                                .unwrap()
                        )
                        .await,
                        [1, 2]
                    );
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("saving_releases_queued", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.target_batch_bytes = policy.request.max_pending_write_bytes;
                    let f = Fixture::new(policy).await;
                    let empty = f.storage.view_with_limits(f.execution).await.unwrap();
                    let view_bytes = f.storage.status().read_bytes;
                    drop(empty);
                    wait_release(&f, 1).await;
                    f.storage
                        .write_records(&[record(
                            &schema(),
                            &Identifier::new("large").unwrap(),
                            &vec![42; 100_000],
                        )])
                        .await
                        .unwrap();
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    assert!(f.storage.status().pending_bytes >= 100_000);
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert_eq!(f.storage.status().read_bytes, view_bytes);
                    assert!(
                        positions(
                            f.storage
                                .query_in(&view, record_query(limits(1, 2048)))
                                .unwrap()
                        )
                        .await
                        .is_empty()
                    );
                    drop(view);
                    wait_release(&f, 2).await;
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cannot_consume_writer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.request.max_pending_write_bytes = NonZeroUsize::new(8192).unwrap();
                    policy.target_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.max_batch_bytes = policy.request.max_pending_write_bytes;
                    policy.budget.read_memory_bytes = NonZeroUsize::new(32 * 1024).unwrap();
                    policy.snapshot_bytes = NonZeroUsize::new(512).unwrap();
                    policy.query_execution.memory_bytes = NonZeroUsize::new(10 * 1024).unwrap();
                    policy.query_decoded_bytes = policy.budget.read_memory_bytes;
                    policy.max_views = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let reading = tokio::spawn(async move { reader.next_page().await });
                    f.data.control.read_entered.notified().await;
                    assert!(matches!(
                        f.storage.view_with_limits(f.execution).await,
                        Err(Error::ResourceExhausted {
                            resource: "storage views"
                        })
                    ));
                    tokio::time::timeout(Duration::from_secs(5), async {
                        for index in 0..16 {
                            write(&f.storage, &format!("later-{index}")).await;
                            assert!(f.storage.status().pending_bytes <= 8192);
                        }
                        f.storage.flush().await.unwrap();
                    })
                    .await
                    .unwrap();
                    assert_eq!(f.storage.status().saved.sequence, 17);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert!(f.storage.status().read_bytes > 0);
                    assert!(f.storage.status().read_bytes <= 32 * 1024);
                    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 0);
                    f.data.records.lock().unwrap().clear();
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    f.data.control.read_release.notify_one();
                    assert_eq!(
                        reading.await.unwrap().unwrap().unwrap()[0]
                            .position
                            .sequence,
                        1
                    );
                    wait_release(&f, 1).await;
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("exhaustion_preserves_pins", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let probe = Fixture::new(config()).await;
                    let view = probe
                        .storage
                        .view_with_limits(probe.execution)
                        .await
                        .unwrap();
                    let bytes = probe.storage.status().read_bytes;
                    drop(view);
                    probe.finish().await;
                    let mut policy = config();
                    policy.budget.read_memory_bytes = NonZeroUsize::new(bytes * 2 - 1).unwrap();
                    let f = Fixture::new(policy).await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    assert!(matches!(
                        f.storage.view_with_limits(f.execution).await,
                        Err(Error::ResourceExhausted {
                            resource: "storage read memory"
                        })
                    ));
                    assert_eq!(view.saved().unwrap().sequence, 0);
                    assert_eq!(f.storage.status().read_bytes, bytes);
                    drop(view);
                    wait_release(&f, 1).await;
                    let next = f.storage.view_with_limits(f.execution).await.unwrap();
                    drop(next);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("typed_page_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    drop(reader);
                    wait_release(&f, 1).await;
                    let retained = f.storage.status().read_bytes;
                    assert!(retained > 0);
                    assert_eq!(page[0].position.sequence, 1);
                    let storage = f.storage.clone();
                    f.finish().await;
                    assert_eq!(storage.status().read_bytes, retained);
                    drop(page);
                    assert_eq!(storage.status().read_bytes, 0);
                });
        });
    }
    {
        scenario_runtime::check("typed_projection_reserves", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    // The Guaranteed View's Ceiling Covers Its Projections.
                    let view = reader.view().unwrap().clone();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let original_charge = view.charged_bytes().unwrap();
                    let projected = page
                        .try_into_value(NonZeroUsize::new(1024).unwrap(), |entries, allowance| {
                            assert_eq!(allowance.get(), 1024);
                            assert!(view.charged_bytes().unwrap() >= original_charge + 1024);
                            Ok(entries
                                .iter()
                                .map(|entry| entry.position.sequence)
                                .collect::<Vec<_>>())
                        })
                        .unwrap();
                    assert_eq!(&*projected, &[1]);
                    projected.check_origin(&f.storage).unwrap();
                    let pointer = projected.as_ptr();
                    let original_charge = view.charged_bytes().unwrap();
                    let response = projected.try_into_response().unwrap();
                    let response = response
                        .try_unwrap()
                        .expect_err("projected loan cannot be detached");
                    assert_eq!(response.as_ptr(), pointer);
                    let charged = view.charged_bytes().unwrap();
                    assert!(charged >= original_charge + std::mem::size_of_val(&response));
                    let cached = response.clone().into_erased();
                    assert_eq!(view.charged_bytes().unwrap(), charged);
                    assert!(cached.clone().restore::<String>().is_none());
                    let restored = cached.clone().restore::<Vec<u64>>().unwrap();
                    assert!(response.ptr_eq(&restored));
                    assert_eq!(restored.as_ptr(), pointer);
                    drop((view, reader));
                    wait_release(&f, 1).await;
                    let storage = f.storage.clone();
                    f.finish().await;
                    let charged = storage.status().read_bytes;
                    assert!(charged > 0);
                    drop(response);
                    assert_eq!(storage.status().read_bytes, charged);
                    drop(restored);
                    assert_eq!(storage.status().read_bytes, charged);
                    drop(cached);
                    assert_eq!(storage.status().read_bytes, 0);
                });
        });
    }
    {
        scenario_runtime::check("refusal_preserves_callback", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(record_query(limits(1, 2048)))
                        .await
                        .unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let mut called = false;
                    let result =
                        page.try_into_value(NonZeroUsize::new(usize::MAX).unwrap(), |_, _| {
                            called = true;
                            Ok(Vec::<u64>::new())
                        });
                    assert!(matches!(result, Err(Error::ResourceExhausted { .. })));
                    assert!(!called);
                    drop(reader);
                    wait_release(&f, 1).await;
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
}

/* === Tests === */
