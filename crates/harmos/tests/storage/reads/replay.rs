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
async fn positions(reader: &mut QueryReplay<StoredRecord, StoredRecord<Vec<u8>>>) -> Vec<u64> {
    tokio::time::timeout(Duration::from_secs(1), async {
        let mut seen = Vec::new();
        while let Some(page) = reader.next_page().await.unwrap() {
            assert!(!page.is_empty());
            assert!(page.len() <= limits().rows.get());
            seen.extend(page.iter().map(|row| row.position.sequence));
        }
        seen
    })
    .await
    .expect("finite replay must not wait for another admission")
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("empty_filtered_schema", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let mut empty = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(empty.boundary().sequence, 0);
                    assert!(positions(&mut empty).await.is_empty());
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
                    f.storage.flush().await.unwrap();
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
                    let mut schemas = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(schemas.boundary().sequence, 2);
                    assert!(positions(&mut schemas).await.is_empty());
                    write(&f.storage, "excluded").await;
                    let mut query = record_query(limits());
                    query.item = Some(Identifier::new("missing").unwrap());
                    let mut filtered = f.storage.replay(query).await.unwrap();
                    assert_eq!(filtered.boundary().sequence, 3);
                    assert!(positions(&mut filtered).await.is_empty());
                    assert_eq!(f.storage.cursor_saved().sequence, 1);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("admissions_use_fixed", || {
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
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(replay.boundary().sequence, 3);
                    write(&f.storage, "later").await;
                    f.storage.flush().await.unwrap();
                    assert_eq!(positions(&mut replay).await, [1, 2]);
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    let mut query = record_query(limits());
                    query.options.after = replay.boundary().sequence;
                    let mut later = f.storage.replay(query).await.unwrap();
                    assert_eq!(positions(&mut later).await, [4]);
                    assert!(positions(&mut replay).await.is_empty());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("requested_ranges_end", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    for item in ["one", "two", "three"] {
                        write(&f.storage, item).await;
                    }
                    f.storage.flush().await.unwrap();
                    let key = schema();
                    let item = Identifier::new("group").unwrap();
                    f.storage
                        .write_records(&[record(&key, &item, &[1]); 3])
                        .await
                        .unwrap();
                    for (after, through, expected) in [
                        (0, 0, vec![]),
                        (0, 2, vec![1, 2]),
                        (1, 3, vec![2, 3]),
                        (2, 5, vec![3, 4, 5]),
                        (4, 6, vec![5, 6]),
                        (5, 5, vec![]),
                    ] {
                        let mut query = record_query(limits());
                        query.options.after = after;
                        query.options.through = Some(through);
                        let mut replay = f.storage.replay(query).await.unwrap();
                        assert_eq!(replay.boundary().sequence, through);
                        assert_eq!(positions(&mut replay).await, expected);
                    }
                    assert_eq!(f.storage.cursor_saved().sequence, 3);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("crosses_bounded_turns", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    // Include the per-entry queue charge while keeping this an unsaved scan.
                    settings.target_batch_bytes = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
                    settings.max_batch_bytes = settings.target_batch_bytes;
                    let f = Fixture::new(settings).await;
                    let key = schema();
                    let excluded = Identifier::new("excluded").unwrap();
                    f.storage
                        .write_records(&vec![record(&key, &excluded, &[1]); 1030])
                        .await
                        .unwrap();
                    let mut query = record_query(limits());
                    query.item = Some(Identifier::new("wanted").unwrap());
                    let mut replay = f.storage.replay(query).await.unwrap();
                    write(&f.storage, "wanted").await;
                    assert_eq!(replay.boundary().sequence, 1030);
                    assert!(positions(&mut replay).await.is_empty());
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("acquisition_never_extend", || {
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
                        tokio::spawn(async move { storage.replay(record_query(limits())).await });
                    f.data.control.view_entered.notified().await;
                    write(&f.storage, "later").await;
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    let mut replay = task.await.unwrap().unwrap();
                    f.storage.flush().await.unwrap();
                    assert_eq!(replay.boundary().sequence, 2);
                    assert_eq!(positions(&mut replay).await, [1, 2]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("saved_page_preserves", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(config()).await;
    write(&f.storage, "saved").await;
    f.storage.flush().await.unwrap();
    write(&f.storage, "unsaved").await;
    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
    f.data.control.block_read.store(true, Ordering::SeqCst);
    {
        let reading = replay.next_page();
        tokio::pin!(reading);
        tokio::select! { biased; _ = &mut reading => panic!("backend is blocked"), _ = f.data.control.read_entered.notified() => {} }
    }
    f.data.control.block_read.store(false, Ordering::SeqCst);
    assert_eq!(positions(&mut replay).await, [1, 2]);
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("saved_page_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "saved").await;
                    f.storage.flush().await.unwrap();
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    let (started, entered) = tokio::sync::oneshot::channel();
                    let (release, waiting) = tokio::sync::oneshot::channel();
                    *f.data.control.accepted_record_read.lock().unwrap() = Some((started, waiting));
                    {
                        let reading = replay.next_page();
                        tokio::pin!(reading);
                        tokio::select! {
                            biased;
                            _ = &mut reading => panic!("physical read is blocked"),
                            result = entered => result.unwrap(),
                        }
                    }
                    assert!(matches!(
                        replay.next_page().await,
                        Err(Error::NotReady {
                            domain: "saved view computation"
                        })
                    ));
                    release.send(()).unwrap();
                    let page = tokio::time::timeout(Duration::from_secs(1), async {
                        loop {
                            match replay.next_page().await {
                                Err(Error::NotReady {
                                    domain: "saved view computation",
                                }) => {
                                    tokio::task::yield_now().await;
                                }
                                result => break result.unwrap().unwrap(),
                            }
                        }
                    })
                    .await
                    .unwrap();
                    assert_eq!(page[0].position.sequence, 1);
                    drop(page);
                    assert!(replay.next_page().await.unwrap().is_none());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("completion_releases_follower", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for complete in [false, true] {
                        let mut settings = config();
                        settings.request.max_pending_write_batches = NonZeroUsize::new(1).unwrap();
                        settings.max_followers = NonZeroUsize::new(1).unwrap();
                        let f = Fixture::new(settings).await;
                        write(&f.storage, "first").await;
                        let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                        if complete {
                            // The final nonempty page releases the lease; no extra None poll needed.
                            assert_eq!(
                                replay.next_page().await.unwrap().unwrap()[0]
                                    .position
                                    .sequence,
                                1
                            );
                        }
                        tokio::time::timeout(Duration::from_secs(1), write(&f.storage, "second"))
                            .await
                            .unwrap();
                        if complete {
                            assert!(positions(&mut replay).await.is_empty());
                            let replacement =
                                f.storage.replay(record_query(limits())).await.unwrap();
                            drop(replacement);
                        } else {
                            assert!(matches!(replay.next_page().await, Err(Error::Gap { .. })));
                        }
                        drop(replay);
                        f.finish().await;
                    }
                });
        });
    }
    {
        scenario_runtime::check("idle_replay_expires", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(settings).await;
                    write(&f.storage, "one").await;
                    let mut idle = f.storage.replay(record_query(limits())).await.unwrap();
                    let mut completed = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(positions(&mut completed).await, [1]);
                    tokio::time::advance(Duration::from_millis(20)).await;
                    assert!(matches!(idle.next_page().await, Err(Error::ViewExpired)));
                    assert!(positions(&mut completed).await.is_empty());
                    drop(idle);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("ranges_never_take", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_followers = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(settings).await;
                    write(&f.storage, "one").await;
                    for (after, through, order) in [
                        (0, Some(2), ReadOrder::Ascending),
                        (1, Some(0), ReadOrder::Ascending),
                        (2, None, ReadOrder::Ascending),
                        (0, None, ReadOrder::Descending),
                    ] {
                        let mut query = record_query(limits());
                        query.options.after = after;
                        query.options.through = through;
                        query.options.order = order;
                        assert!(matches!(
                            f.storage.replay(query).await,
                            Err(Error::InvalidInput { .. })
                        ));
                    }
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(positions(&mut replay).await, [1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("replay_never_certify", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "one").await;
                    f.data.control.permanent.store(true, Ordering::SeqCst);
                    assert!(f.storage.flush().await.is_err());
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(positions(&mut replay).await, [1]);
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    assert!(f.storage.status().failure.is_some());
                    f.data.control.permanent.store(false, Ordering::SeqCst);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("budget_never_complete", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "one").await;
                    let mut replay = f
                        .storage
                        .replay(refusing_record_query(limits()))
                        .await
                        .unwrap();
                    assert!(matches!(
                        replay.next_page().await,
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    assert_eq!(positions(&mut replay).await, [1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("acquisition_releases_retention", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.max_followers = NonZeroUsize::new(1).unwrap();
                    let f = Fixture::new(settings).await;
                    write(&f.storage, "one").await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let task =
                        tokio::spawn(async move { storage.replay(record_query(limits())).await });
                    f.data.control.view_entered.notified().await;
                    task.abort();
                    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
                    f.data.control.block_view.store(false, Ordering::SeqCst);
                    f.data.control.view_release.notify_one();
                    f.storage.flush().await.unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    let mut replacement = f.storage.replay(record_query(limits())).await.unwrap();
                    assert_eq!(positions(&mut replacement).await, [1]);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("files_replay_selected", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
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
                    let mut schemas = f.storage.replay(schema_query(limits())).await.unwrap();
                    let mut streams = f.storage.replay(stream_query(limits())).await.unwrap();
                    let mut files = f.storage.replay(file_query(limits())).await.unwrap();
                    assert_eq!(schemas.boundary().sequence, 3);
                    assert_eq!(streams.boundary().sequence, 3);
                    assert_eq!(files.boundary().sequence, 3);
                    assert_eq!(schemas.next_page().await.unwrap().unwrap()[0].key, key);
                    assert_eq!(streams.next_page().await.unwrap().unwrap()[0].sequence, 99);
                    assert_eq!(&*files.next_page().await.unwrap().unwrap(), &[reference]);
                    assert!(schemas.next_page().await.unwrap().is_none());
                    assert!(streams.next_page().await.unwrap().is_none());
                    assert!(files.next_page().await.unwrap().is_none());
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("close_releases_unread", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "unread").await;
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    f.storage.flush().await.unwrap();
                    assert!(f.storage.status().pending_bytes > 0);
                    tokio::time::timeout(Duration::from_secs(1), f.storage.close())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(f.storage.status().pending_bytes, 0);
                    assert!(matches!(
                        replay.next_page().await,
                        Err(Error::Closing { .. })
                    ));
                    drop(replay);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("replay_page_retains", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "live").await;
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    let page = replay.next_page().await.unwrap().unwrap();
                    drop(replay);
                    super::views::wait_release(&f, 1).await;
                    assert_eq!(page[0].position.sequence, 1);
                    let retained = f.storage.status().read_bytes;
                    let projected = page
                        .try_project(NonZeroUsize::new(64).unwrap(), |rows, _| {
                            Ok(rows
                                .iter()
                                .map(|row| row.position.sequence)
                                .collect::<Vec<_>>())
                        })
                        .unwrap();
                    assert_eq!(&projected[..], &[1]);
                    assert!(f.storage.status().read_bytes > retained);
                    drop(projected);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    assert!(retained > 0);
                    let storage = f.storage.clone();
                    f.finish().await;
                    assert_eq!(storage.status().read_bytes, retained);
                    drop(page);
                    assert_eq!(storage.status().read_bytes, 0);
                });
        });
    }
    {
        scenario_runtime::check("projected_replay_preserves", || {
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
                    write(&f.storage, "excluded").await;
                    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                    let decoded = calls.clone();
                    let mut query = RecordQuery::new(schema(), move |bytes, allowance| {
                        decoded.fetch_add(1, Ordering::SeqCst);
                        decode_bytes(bytes, allowance)
                    });
                    query.options.page = Some(limits());
                    query.options.decoded_bytes = Some(limits().bytes);
                    query.options.through = Some(2);
                    let mut replay = f.storage.replay(query).await.unwrap();
                    assert_eq!(replay.boundary().sequence, 2);
                    let allowance = NonZeroUsize::new(limits().bytes.get() + 1).unwrap();
                    for expected in [1, 2] {
                        let mut pointer = 0;
                        let refused = replay
                            .next_page_projected::<Vec<u64>>(allowance, |rows, _| {
                                assert_eq!(rows[0].position.sequence, expected);
                                pointer = rows.as_ptr() as usize;
                                Err(Error::ResourceExhausted {
                                    resource: "projection refusal",
                                })
                            })
                            .await;
                        assert!(matches!(refused, Err(Error::ResourceExhausted { .. })));
                        assert_eq!(calls.load(Ordering::SeqCst), expected as usize);
                        let oversized = NonZeroUsize::new(
                            storage_settings().budget.read_memory_bytes.get() + 1,
                        )
                        .unwrap();
                        assert!(
                            replay
                                .next_page_projected::<Vec<u64>>(oversized, |_, _| panic!(
                                    "not admitted"
                                ))
                                .await
                                .is_err()
                        );
                        {
                            let work =
                                replay.next_page_projected::<Vec<u64>>(allowance, |rows, _| {
                                    assert_eq!(rows.as_ptr() as usize, pointer);
                                    panic!("projection callback panicked");
                                });
                            let mut work = std::pin::pin!(work);
                            let panicked =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    let mut context =
                                        std::task::Context::from_waker(std::task::Waker::noop());
                                    std::future::Future::poll(work.as_mut(), &mut context)
                                }));
                            assert!(panicked.is_err());
                        }
                        let value = replay
                            .next_page_projected(allowance, |rows, _| {
                                assert_eq!(rows.as_ptr() as usize, pointer);
                                Ok(rows
                                    .iter()
                                    .map(|row| row.position.sequence)
                                    .collect::<Vec<_>>())
                            })
                            .await
                            .unwrap()
                            .unwrap();
                        assert_eq!(&**value, &[expected]);
                        assert_eq!(calls.load(Ordering::SeqCst), expected as usize);
                        value.check_origin(&f.storage).unwrap();
                    }
                    assert!(
                        replay
                            .next_page_projected::<Vec<u64>>(allowance, |_, _| panic!(
                                "already exhausted"
                            ))
                            .await
                            .unwrap()
                            .is_none()
                    );
                    drop(replay);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projected_replay_spends", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    write(&f.storage, "one").await;
                    let mut query = record_query(limits());
                    query.options.execution = Some(QueryLimits {
                        timeout: Duration::from_millis(400),
                        ..storage_settings().query_execution
                    });
                    let mut replay = f.storage.replay(query).await.unwrap();
                    let allowance = NonZeroUsize::new(64).unwrap();
                    let mut pointer = 0;
                    assert!(
                        replay
                            .next_page_projected::<u64>(allowance, |rows, _| {
                                pointer = rows.as_ptr() as usize;
                                std::thread::sleep(Duration::from_millis(250));
                                Err(Error::ResourceExhausted {
                                    resource: "projection refusal",
                                })
                            })
                            .await
                            .is_err()
                    );
                    let retained = f.storage.status().read_bytes;
                    let overrun = replay
                        .next_page_projected_shared(allowance, |rows, _| {
                            assert_eq!(rows.as_ptr() as usize, pointer);
                            std::thread::sleep(Duration::from_millis(250));
                            Ok(rows[0].position.sequence)
                        })
                        .await;
                    assert!(matches!(
                        overrun,
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let retry = replay
                        .next_page_projected_shared::<u64>(allowance, |_, _| {
                            panic!("original clock exhausted")
                        })
                        .await;
                    assert!(matches!(
                        retry,
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    drop(replay);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_keeps_lease", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    settings.view_idle_timeout = Duration::from_millis(10);
                    let f = Fixture::new(settings).await;
                    write(&f.storage, "one").await;
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    let allowance = NonZeroUsize::new(64).unwrap();
                    assert!(
                        replay
                            .next_page_projected::<u64>(allowance, |_, _| Err(Error::NotReady {
                                domain: "projection refusal"
                            }))
                            .await
                            .is_err()
                    );
                    tokio::time::advance(Duration::from_millis(20)).await;
                    assert!(matches!(
                        replay
                            .next_page_projected::<u64>(allowance, |_, _| panic!("lease expired"))
                            .await,
                        Err(Error::ViewExpired)
                    ));
                    drop(replay);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("refusal_retains_replay", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = config();
                    // Admit Two Full Views Before Exhausting Their Shared Output Ledger.
                    settings.budget.read_memory_bytes = NonZeroUsize::new(64 * 1024).unwrap();
                    settings.snapshot_bytes = NonZeroUsize::new(512).unwrap();
                    settings.query_decoded_bytes = limits().bytes;
                    // The Backend Preclaims All Snapshot Map Nodes, Independently Of Page Size.
                    settings.query_execution.memory_bytes = NonZeroUsize::new(8 * 1024).unwrap();
                    let capacity = settings.budget.read_memory_bytes.get();
                    let f = Fixture::new(settings).await;
                    write(&f.storage, "one").await;
                    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
                    let allowance = NonZeroUsize::new(std::mem::size_of::<Vec<u8>>() + 3).unwrap();
                    let mut pointer = 0;
                    assert!(
                        replay
                            .next_page_projected_shared::<Vec<u8>>(allowance, |rows, _| {
                                pointer = rows.as_ptr() as usize;
                                Err(Error::NotReady {
                                    domain: "projection refusal",
                                })
                            })
                            .await
                            .is_err()
                    );

                    // Competing Actual Payload Loans On The Same Ledger. The
                    // Competing View Leaves First: An Open View's Reservation
                    // Never Refuses View-Less Charges, Actual Loans Do.
                    let mut competing = f.storage.replay(record_query(limits())).await.unwrap();
                    let page = competing.next_page().await.unwrap().unwrap();
                    drop(competing);
                    super::views::wait_release(&f, 2).await;
                    assert_eq!(
                        harmos::MemorySize::memory_size(&page[0].payload),
                        allowance.get()
                    );
                    let mut loans = Vec::new();
                    loop {
                        assert!(loans.len() <= capacity / allowance.get());
                        match page.try_project(allowance, |rows, _| Ok(rows[0].payload.clone())) {
                            Ok(loan) => loans.push(loan),
                            Err(Error::ResourceExhausted {
                                resource: "storage read memory",
                            }) => break,
                            Err(error) => panic!("unexpected competing loan refusal: {error}"),
                        }
                    }
                    drop(loans.pop().expect("actual payload loan can be released"));
                    let retained = f.storage.status().read_bytes;
                    let mut called = false;
                    let refused = replay
                        .next_page_projected_shared(allowance, |rows, _| {
                            called = true;
                            assert_eq!(rows.as_ptr() as usize, pointer);
                            Ok(rows[0].payload.clone())
                        })
                        .await;
                    assert!(
                        called,
                        "projection was admitted before shared-header refusal"
                    );
                    assert!(matches!(
                        refused,
                        Err(Error::ResourceExhausted {
                            resource: "storage read memory"
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, retained);
                    drop(loans);
                    drop(page);

                    let shared = replay
                        .next_page_projected_shared(allowance, |rows, _| {
                            assert_eq!(rows.as_ptr() as usize, pointer);
                            Ok(rows[0].payload.clone())
                        })
                        .await
                        .unwrap()
                        .unwrap();
                    let clone = shared.clone();
                    assert_eq!(shared.as_ptr(), clone.as_ptr());
                    assert_eq!(&**clone, &[1, 2, 3]);
                    assert!(
                        replay
                            .next_page_projected_shared::<Vec<u8>>(allowance, |_, _| panic!(
                                "completed"
                            ))
                            .await
                            .unwrap()
                            .is_none()
                    );
                    drop(replay);
                    let storage = f.storage.clone();
                    f.finish().await;
                    let retained = storage.status().read_bytes;
                    assert!(retained > 0);
                    drop(shared);
                    assert_eq!(storage.status().read_bytes, retained);
                    drop(clone);
                    assert_eq!(storage.status().read_bytes, 0);
                });
        });
    }
}

#[tokio::test]
async fn accepted_replay_projects_after_saved_view_release() {
    let f = Fixture::new(config()).await;
    write(&f.storage, "accepted").await;
    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
    super::views::wait_release(&f, 1).await;
    let value = replay
        .next_page_projected(NonZeroUsize::new(64).unwrap(), |rows, _| {
            assert_eq!(rows.len(), 1);
            Ok(rows[0].position.sequence)
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*value, 1);
    drop(value);
    assert!(replay.next_page().await.unwrap().is_none());
    drop(replay);
    f.finish().await;
}

#[tokio::test]
async fn accepted_replay_waits_for_shared_capacity() {
    let f = Fixture::new(config()).await;
    write(&f.storage, "accepted").await;
    let mut replay = f.storage.replay(record_query(limits())).await.unwrap();
    super::views::wait_release(&f, 1).await;
    let occupied = f.storage.status().read_bytes;
    let probe = f.storage.reserve_arrow_memory(0).unwrap();
    let overhead = f.storage.status().read_bytes - occupied;
    drop(probe);
    let capacity = config().budget.read_memory_bytes.get();
    let external = f
        .storage
        .reserve_arrow_memory(capacity - occupied - overhead)
        .unwrap();
    let mut next = Box::pin(replay.next_page());
    assert!(matches!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(std::future::Future::poll(
            next.as_mut(),
            cx
        )))
        .await,
        std::task::Poll::Pending
    ));
    drop(external);
    let page = next.await.unwrap().unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].position.sequence, 1);
    drop(page);
    assert!(replay.next_page().await.unwrap().is_none());
    drop(replay);
    f.finish().await;
}
