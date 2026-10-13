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
fn limits() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(2).unwrap(),
        bytes: NonZeroUsize::new(2048).unwrap(),
    }
}
async fn page(reader: &mut QueryFollower<StoredRecord, StoredRecord<Vec<u8>>>) -> Vec<u64> {
    tokio::time::timeout(Duration::from_secs(1), reader.next_page())
        .await
        .unwrap()
        .unwrap()
        .iter()
        .map(|row| row.position.sequence)
        .collect()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("history_unsaved_live", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(config()).await;
    write(&f.storage, "saved").await;
    f.storage.flush().await.unwrap();
    write(&f.storage, "unsaved").await;
    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
    assert_eq!(reader.saved_boundary().sequence, 1);
    assert_eq!(reader.accepted_boundary().sequence, 2);
    assert_eq!(page(&mut reader).await, [1]);
    f.storage.flush().await.unwrap();
    assert!(f.storage.status().pending_bytes > 0);
    write(&f.storage, "live").await;
    assert_eq!(page(&mut reader).await, [2, 3]);
    // Reading live data does not remove the writer's unsaved ownership.
    assert_eq!(f.storage.cursor_saved().sequence, 2);
    assert!(f.storage.status().pending_bytes > 0);
    f.storage.flush().await.unwrap();
    assert_eq!(f.storage.status().pending_bytes, 0);
    {
        let waiting = reader.next_page();
        tokio::pin!(waiting);
        tokio::select! { biased; _ = &mut waiting => panic!("save must not reemit"), _ = tokio::task::yield_now() => {} }
        write(&f.storage, "next").await;
        assert_eq!(waiting.await.unwrap()[0].position.sequence, 4);
    }
    drop(reader);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("pressure_reports_gap", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.queue_capacity = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "first").await;
                    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(1), write(&f.storage, "second"))
                        .await
                        .unwrap();
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::Gap {
                            requested: 1,
                            available: 2
                        })
                    ));
                    drop(reader);
                    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
                    assert_eq!(page(&mut reader).await, [1]);
                    assert_eq!(page(&mut reader).await, [2]);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("partial_group_page", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let key = schema();
                    let item = Identifier::new("group").unwrap();
                    f.storage
                        .write_records(&[record(&key, &item, &[1]); 5])
                        .await
                        .unwrap();
                    let mut reader = f
                        .storage
                        .follow(refusing_record_query(limits()))
                        .await
                        .unwrap();
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    f.storage.flush().await.unwrap();
                    assert_eq!(page(&mut reader).await, [1, 2]);
                    assert!(f.storage.status().pending_bytes > 0);
                    assert_eq!(page(&mut reader).await, [3, 4]);
                    assert!(f.storage.status().pending_bytes > 0);
                    assert_eq!(page(&mut reader).await, [5]);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("wait_never_consume", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(config()).await;
    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
    {
        let waiting = reader.next_page();
        tokio::pin!(waiting);
        tokio::select! { biased; _ = &mut waiting => panic!("no records"), _ = tokio::task::yield_now() => {} }
    }
    write(&f.storage, "after-cancel").await;
    assert_eq!(page(&mut reader).await, [1]);
    drop(reader);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("expiry_releases_retention", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let mut policy = config();
    policy.view_idle_timeout = Duration::from_millis(10);
    let f = Fixture::new(policy).await;
    let mut idle = f.storage.follow(record_query(limits())).await.unwrap();
    let mut active = f.storage.follow(record_query(limits())).await.unwrap();
    {
        let waiting = active.next_page();
        tokio::pin!(waiting);
        tokio::select! { biased; _ = &mut waiting => panic!("no records"), _ = tokio::task::yield_now() => {} }
        write(&f.storage, "retained").await;
        f.storage.flush().await.unwrap();
        tokio::time::advance(Duration::from_millis(20)).await;
        assert!(matches!(idle.next_page().await, Err(Error::ViewExpired)));
        assert_eq!(waiting.await.unwrap()[0].position.sequence, 1);
    }
    assert_eq!(f.storage.status().pending_bytes, 0);
    drop(idle);
    drop(active);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("count_limit_drop", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_followers = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    let reader = f.storage.follow(record_query(limits())).await.unwrap();
                    assert!(matches!(
                        f.storage.follow(record_query(limits())).await,
                        Err(Error::ResourceExhausted { .. })
                    ));
                    f.storage.flush().await.unwrap();
                    assert!(f.storage.status().pending_bytes > 0);
                    drop(reader);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    let reader = f.storage.follow(record_query(limits())).await.unwrap();
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("acquisition_retains_admissions", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    write(&f.storage, "unsaved").await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let task =
                        tokio::spawn(async move { storage.follow(record_query(limits())).await });
                    f.data.control.view_entered.notified().await;
                    write(&f.storage, "during-acquisition").await;
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    let mut reader = task.await.unwrap().unwrap();
                    f.storage.flush().await.unwrap();
                    assert_eq!(reader.saved_boundary().sequence, 1);
                    assert_eq!(reader.accepted_boundary().sequence, 2);
                    assert_eq!(page(&mut reader).await, [1]);
                    assert_eq!(page(&mut reader).await, [2, 3]);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("acquisition_releases_follower", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_followers = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    write(&f.storage, "one").await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let task =
                        tokio::spawn(async move { storage.follow(record_query(limits())).await });
                    f.data.control.view_entered.notified().await;
                    task.abort();
                    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    let reader = f.storage.follow(record_query(limits())).await.unwrap();
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("follower_drains_storage", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
                    let waiting = tokio::spawn(async move { reader.next_page().await });
                    tokio::task::yield_now().await;
                    f.storage.close().await.unwrap();
                    assert!(matches!(waiting.await.unwrap(), Err(Error::Closing { .. })));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("filters_resume_skip", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "skip").await;
                    let mut query = record_query(limits());
                    query.options.after = 1;
                    query.item = Some(Identifier::new("wanted").unwrap());
                    let mut reader = f.storage.follow(query).await.unwrap();
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
                    write(&f.storage, "skip").await;
                    write(&f.storage, "wanted").await;
                    assert_eq!(page(&mut reader).await, [4]);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("persistence_failure_reported", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
                    write(&f.storage, "unsaved").await;
                    f.data.control.permanent.store(true, Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    assert_eq!(page(&mut reader).await, [1]);
                    assert!(matches!(
                        reader.next_page().await,
                        Err(Error::Persistence { .. })
                    ));
                    assert!(f.storage.status().failure.is_some());
                    f.data.control.permanent.store(false, Ordering::SeqCst);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("selectors_never_claim", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut policy = config();
                    policy.max_followers = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(policy).await;
                    for (order, through, after) in [
                        (ReadOrder::Descending, None, 0),
                        (ReadOrder::Ascending, Some(0), 0),
                        (ReadOrder::Ascending, None, 1),
                    ] {
                        let mut query = record_query(limits());
                        query.options.order = order;
                        query.options.through = through;
                        query.options.after = after;
                        assert!(matches!(
                            f.storage.follow(query).await,
                            Err(Error::InvalidInput { .. })
                        ));
                    }
                    let reader = f.storage.follow(record_query(limits())).await.unwrap();
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("budget_never_advance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "first").await;
                    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
                    let huge = ReadLimits {
                        bytes: NonZeroUsize::new(usize::MAX).unwrap(),
                        ..limits()
                    };
                    let mut rejected = record_query(huge);
                    rejected.options.decoded_bytes = Some(limits().bytes);
                    assert!(f.storage.follow(rejected).await.is_err());
                    assert_eq!(page(&mut reader).await, [1]);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("schemas_streams_file", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut schemas = f.storage.follow(schema_query(limits())).await.unwrap();
                    let mut streams = f.storage.follow(stream_query(limits())).await.unwrap();
                    let mut files = f.storage.follow(file_query(limits())).await.unwrap();
                    let key = schema();
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
                    assert_eq!(schemas.next_page().await.unwrap()[0].key, key);
                    let stream = Identifier::new("voltage").unwrap();
                    let item = Identifier::new("sample").unwrap();
                    f.storage
                        .write_stream(&[StreamWrite {
                            stream: &stream,
                            sequence: 99,
                            record: record(&key, &item, &[1]),
                        }])
                        .await
                        .unwrap();
                    assert_eq!(streams.next_page().await.unwrap()[0].sequence, 99);
                    let reference = f
                        .storage
                        .write_file(
                            FileMetadata {
                                id: Identifier::new("file").unwrap(),
                                scope: None,
                                media_type: "text/plain".into(),
                                expected_length: Some(3),
                                expected_sha256: None,
                            },
                            b"abc",
                        )
                        .await
                        .unwrap();
                    assert_eq!(&*files.next_page().await.unwrap(), &[reference]);
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    drop((schemas, streams, files));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("follower_retries_page", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "one").await;
                    let mut reader = f.storage.follow(record_query(limits())).await.unwrap();
                    let allowance = NonZeroUsize::new(64).unwrap();
                    let mut pointer = 0;
                    assert!(
                        reader
                            .next_page_projected::<u64>(allowance, |rows, _| {
                                pointer = rows.as_ptr() as usize;
                                Err(Error::NotReady {
                                    domain: "projection refusal",
                                })
                            })
                            .await
                            .is_err()
                    );
                    write(&f.storage, "two").await;
                    let first = reader.next_page().await.unwrap();
                    assert_eq!(first.as_ptr() as usize, pointer);
                    assert_eq!(first[0].position.sequence, 1);
                    drop(first);
                    let second = reader
                        .next_page_projected_shared(allowance, |rows, _| {
                            Ok(rows[0].position.sequence)
                        })
                        .await
                        .unwrap();
                    assert_eq!(*second, 2);
                    second.check_origin(&f.storage).unwrap();
                    drop(reader);
                    drop(second);
                    f.finish().await;
                });
        });
    }
}
