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

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("projection_retains_read", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.storage
                        .write_object(&Document { id: 7, value: 42 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let original = f
                        .storage
                        .read_object::<Document>(
                            &7,
                            ObjectVersion::Latest,
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    until(|| f.data.control.releases.load(Ordering::SeqCst) == 1).await;
                    let input_charge = f.storage.status().read_bytes;
                    let projected = original
                        .try_map(NonZeroUsize::new(1024).unwrap(), |original, allowance| {
                            // Reentrant inspection proves callback execution holds no manager lock.
                            assert!(
                                f.storage.status().read_bytes >= input_charge + allowance.get()
                            );
                            assert_eq!(original.value.value, 42);
                            Ok(original.value)
                        })
                        .unwrap();
                    assert_eq!(projected.value, 42);
                    assert!(f.storage.status().read_bytes >= projected.memory_size());
                    assert!(f.storage.status().read_bytes < input_charge + 1024);
                    drop(projected);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("error_panic_release", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let settings = StorageSettings::default();
                    let f = Fixture::with_settings(StorageSettings {
                        query_decoded_bytes_ceiling: settings.read_memory_bytes,
                        ..settings
                    })
                    .await;
                    f.storage
                        .write_object(&Document { id: 7, value: 42 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    for outcome in 0..5 {
                        let original = f
                            .storage
                            .read_object::<Document>(
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(4096).unwrap(),
                            )
                            .await
                            .unwrap()
                            .unwrap();
                        until(|| f.data.control.releases.load(Ordering::SeqCst) == outcome + 1)
                            .await;
                        let called = AtomicBool::new(false);
                        match outcome {
                            0 => {
                                let result = original.try_map::<Document>(
                                    StorageSettings::default().read_memory_bytes,
                                    |_, _| {
                                        called.store(true, Ordering::SeqCst);
                                        panic!("capacity refusal must precede callback")
                                    },
                                );
                                assert!(matches!(
                                    result,
                                    Err(Error::ResourceExhausted {
                                        resource: "storage read memory"
                                    })
                                ));
                                assert!(!called.load(Ordering::SeqCst));
                            }
                            1 => {
                                let result = original.try_map::<Document>(
                                    NonZeroUsize::new(1024).unwrap(),
                                    |_, _| {
                                        Err(Error::InvalidInput {
                                            field: "projection",
                                            reason: "refused",
                                        })
                                    },
                                );
                                assert!(matches!(
                                    result,
                                    Err(Error::InvalidInput {
                                        field: "projection",
                                        ..
                                    })
                                ));
                            }
                            2 => {
                                let result = original.try_map(
                                    NonZeroUsize::new(1024).unwrap(),
                                    |_, allowance| {
                                        Ok(Vec::<u8>::with_capacity(allowance.get() + 1))
                                    },
                                );
                                assert!(matches!(
                                    result,
                                    Err(Error::ResourceExhausted {
                                        resource: "query decoded memory"
                                    })
                                ));
                            }
                            3 => {
                                let result =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        original.try_map::<Document>(
                                            NonZeroUsize::new(1024).unwrap(),
                                            |_, _| panic!("projection panic"),
                                        )
                                    }));
                                assert!(result.is_err());
                            }
                            4 => {
                                let requested = NonZeroUsize::new(
                                    StorageSettings::default().read_memory_bytes.get() + 1,
                                )
                                .unwrap();
                                let result = original.try_map::<Document>(requested, |_, _| {
                                    called.store(true, Ordering::SeqCst);
                                    panic!("configured ceiling refusal must precede callback")
                                });
                                assert!(matches!(
                                    result,
                                    Err(Error::ResourceExhausted {
                                        resource: "query decoded memory"
                                    })
                                ));
                                assert!(!called.load(Ordering::SeqCst));
                            }
                            _ => unreachable!(),
                        }
                        assert_eq!(f.storage.status().read_bytes, 0);
                    }
                    f.storage
                        .write_object(&Document { id: 8, value: 43 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("failure_preserves_query", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.append(2).await;
                    f.storage.flush().await.unwrap();
                    let decoded = Arc::new(AtomicUsize::new(0));
                    let calls = Arc::clone(&decoded);
                    let mut query = RecordQuery::new(schema(), move |bytes, allowance| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        decode(bytes, allowance)
                    });
                    query.options.page = Some(limits(2, 4096));
                    let mut result = f.storage.query(query).await.unwrap();
                    let original = result.view().unwrap().saved().unwrap();
                    let pointer = AtomicUsize::new(0);
                    let failed = result
                        .next_page_projected::<u64>(NonZeroUsize::new(64).unwrap(), |rows, _| {
                            pointer.store(rows.as_ptr() as usize, Ordering::SeqCst);
                            assert_eq!(
                                rows.iter().map(|row| row.payload).collect::<Vec<_>>(),
                                [1, 2]
                            );
                            Err(Error::InvalidInput {
                                field: "domain projection",
                                reason: "retry",
                            })
                        })
                        .await;
                    assert!(matches!(
                        failed,
                        Err(Error::InvalidInput {
                            field: "domain projection",
                            ..
                        })
                    ));
                    assert_eq!(result.view().unwrap().saved().unwrap(), original);
                    assert_eq!(decoded.load(Ordering::SeqCst), 2);
                    let retained = f.storage.status().read_bytes;
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let value = tokio::time::timeout(
                        Duration::from_secs(1),
                        result.next_page_projected(NonZeroUsize::new(64).unwrap(), |rows, _| {
                            assert_eq!(rows.as_ptr() as usize, pointer.load(Ordering::SeqCst));
                            assert!(f.storage.status().read_bytes >= retained);
                            Ok(rows.iter().map(|row| row.payload).sum::<u64>())
                        }),
                    )
                    .await
                    .expect("retry must not fetch the retained page")
                    .unwrap()
                    .unwrap();
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    assert_eq!(decoded.load(Ordering::SeqCst), 2);
                    assert_eq!(*value, 3);
                    assert!(result.next_page().await.unwrap().is_none());
                    drop(value);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("origin_refuses_manager", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let other = Fixture::new().await;
                    assert_eq!(f.data.history(), other.data.history());
                    let allowance = NonZeroUsize::new(1024).unwrap();
                    let value = f
                        .storage
                        .produce(allowance, |_| Ok(7_u64))
                        .unwrap()
                        .try_into_value()
                        .unwrap();
                    assert!(value.check_origin(&f.storage).is_ok());
                    assert!(value.check_origin(&f.storage.clone()).is_ok());
                    assert!(matches!(
                        value.check_origin(&other.storage),
                        Err(Error::InvalidInput {
                            field: "query projection",
                            reason: "input belongs to another read ledger"
                        })
                    ));
                    let shared = value.try_into_shared().unwrap();
                    let retained = shared.clone();
                    let original = f.storage.status().read_bytes;
                    let foreign = other.storage.status().read_bytes;
                    assert!(retained.check_origin(&f.storage).is_ok());
                    let called = AtomicBool::new(false);
                    let refused = shared.check_origin(&other.storage).and_then(|()| {
                        other.storage.produce(allowance, |_| {
                            called.store(true, Ordering::SeqCst);
                            Ok(9_u64)
                        })
                    });
                    assert!(matches!(
                        refused,
                        Err(Error::InvalidInput {
                            field: "query projection",
                            reason: "input belongs to another read ledger"
                        })
                    ));
                    assert!(!called.load(Ordering::SeqCst));
                    assert_eq!(f.storage.status().read_bytes, original);
                    assert_eq!(other.storage.status().read_bytes, foreign);
                    assert_eq!(*retained, 7);
                    drop((shared, retained));
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                    other.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("transfer_keeps_buffer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let production = f
                        .storage
                        .produce(NonZeroUsize::new(4096).unwrap(), |_| {
                            Ok(Vec::<u8>::with_capacity(128))
                        })
                        .unwrap();
                    let original = production.value().as_ptr();
                    let charged = f.storage.status().read_bytes;
                    let value = production.try_into_value().unwrap();
                    assert_eq!(value.as_ptr(), original);
                    assert_eq!(f.storage.status().read_bytes, charged);
                    assert_eq!(f.storage.cursor_accepted().sequence, 0);
                    assert_eq!(f.storage.cursor_saved().sequence, 0);
                    let shared = value.try_into_shared().unwrap();
                    assert_eq!(shared.as_ptr(), original);
                    let shared_charge = f.storage.status().read_bytes;
                    let clone = shared.clone();
                    assert_eq!(f.storage.status().read_bytes, shared_charge);
                    drop(shared);
                    assert_eq!(clone.as_ptr(), original);
                    assert_eq!(f.storage.status().read_bytes, shared_charge);
                    drop(clone);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("composition_keeps_inputs", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(1).await;
                    f.append(2).await;
                    f.storage.flush().await.unwrap();
                    let view = f
                        .storage
                        .view_with_limits(StorageSettings::default().query_execution)
                        .await
                        .unwrap();
                    let mut reader = f
                        .storage
                        .query_in(&view, RecordQuery::new(schema(), decode))
                        .unwrap();
                    let first = reader.next_page().await.unwrap().unwrap();
                    let before = f.storage.status().read_bytes;
                    let value = first
                        .try_into_value(NonZeroUsize::new(4096).unwrap(), |records, _| {
                            assert!(f.storage.status().read_bytes >= before + 4096);
                            Ok(records
                                .into_iter()
                                .map(|record| record.payload)
                                .collect::<Vec<_>>())
                        })
                        .unwrap();
                    let before = f.storage.status().read_bytes;
                    let combined = reader
                        .next_page_projected(NonZeroUsize::new(4096).unwrap(), |records, _| {
                            assert!(f.storage.status().read_bytes >= before + 4096);
                            let mut values = (*value).clone();
                            values.extend(records.iter().map(|record| record.payload));
                            Ok(values)
                        })
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(&**value, &[1]);
                    assert_eq!(&**combined, &[1, 2]);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    drop((value, combined, reader, view));
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("composition_refuses_different", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let first = Fixture::new().await;
                    let second = Fixture::new().await;
                    first.append(1).await;
                    second.append(2).await;
                    first.storage.flush().await.unwrap();
                    second.storage.flush().await.unwrap();
                    let mut left = first
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    let mut right = second
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    let value = left
                        .next_page()
                        .await
                        .unwrap()
                        .unwrap()
                        .try_into_value(NonZeroUsize::new(4096).unwrap(), |records, _| {
                            Ok(records
                                .into_iter()
                                .map(|record| record.payload)
                                .collect::<Vec<_>>())
                        })
                        .unwrap();
                    right.retain_view().unwrap();
                    let page = right.next_page().await.unwrap().unwrap();
                    let result = right.project(
                        &value,
                        NonZeroUsize::new(4096).unwrap(),
                        |_, _| -> Result<Vec<u64>> {
                            panic!("different managers must be refused before projection");
                        },
                    );
                    assert!(matches!(
                        result,
                        Err(Error::InvalidInput {
                            field: "query projection",
                            ..
                        })
                    ));
                    assert_eq!(&**value, &[1]);
                    assert_eq!(page[0].payload, 2);
                    drop((value, page, left, right));
                    until(|| {
                        first.storage.status().read_bytes == 0
                            && second.storage.status().read_bytes == 0
                    })
                    .await;
                    first.finish().await;
                    second.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancelled_page_projection", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new().await;
    f.append(1).await;
    f.storage.flush().await.unwrap();
    let mut reader = f
        .storage
        .query(RecordQuery::new(schema(), decode))
        .await
        .unwrap();
    f.data.control.block_read.store(true, Ordering::SeqCst);
    let called = AtomicBool::new(false);
    let mut projection = Box::pin(reader.next_page_projected(
        NonZeroUsize::new(4096).unwrap(),
        |records, _| {
            called.store(true, Ordering::SeqCst);
            Ok(records
                .iter()
                .map(|record| record.payload)
                .collect::<Vec<_>>())
        },
    ));
    tokio::select! {
        result = &mut projection => panic!("projection completed before release: {result:?}"),
        () = f.data.control.read_entered.notified() => {}
    }
    assert!(f.storage.status().read_bytes >= 4096);
    assert!(!called.load(Ordering::SeqCst));
    drop(projection);
    drop(reader);
    f.data.control.block_read.store(false, Ordering::SeqCst);
    until(|| f.storage.status().read_bytes == 0).await;
    f.finish().await;

});
        });
    }
    {
        scenario_runtime::check("retains_page_refusal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(7).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), |bytes, allowance| {
                        if bytes.len().saturating_add(std::mem::size_of::<Vec<u8>>())
                            > allowance.get()
                        {
                            return Err(Error::ResourceExhausted {
                                resource: "fixture payload decode",
                            });
                        }
                        Ok(bytes.to_vec())
                    });
                    query.options.page = Some(limits(1, 4096));
                    let mut reader = f.storage.query(query).await.unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let original = page[0].payload.as_ptr();
                    let before = f.storage.status().read_bytes;
                    let failed = page.try_project(
                        NonZeroUsize::new(4096).unwrap(),
                        |records, _| -> Result<Vec<u64>> {
                            assert_eq!(records[0].payload.as_ptr(), original);
                            assert!(f.storage.status().read_bytes >= before + 4096);
                            Err(Error::InvalidInput {
                                field: "fixture raw decode",
                                reason: "retry original page",
                            })
                        },
                    );
                    assert!(failed.is_err());
                    assert_eq!(f.storage.status().read_bytes, before);
                    let decoded = page
                        .try_project(NonZeroUsize::new(4096).unwrap(), |records, allowance| {
                            records
                                .iter()
                                .map(|record| decode(&record.payload, allowance))
                                .collect::<Result<Vec<_>>>()
                        })
                        .unwrap();
                    assert_eq!(&*decoded, &[7]);
                    assert_eq!(page[0].payload.as_ptr(), original);
                    drop((page, decoded, reader));
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_preserves_rows", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let first = f.append(7).await;
                    let second = f.append(8).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.page = Some(limits(1, 4096));
                    let mut reader = f.storage.query(query).await.unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let original = page.as_ptr();
                    let input_charge = f.storage.status().read_bytes;
                    let views = f.data.control.query_views.load(Ordering::SeqCst);

                    let called = AtomicBool::new(false);
                    let excessive = NonZeroUsize::new(
                        StorageSettings::default().query_decoded_bytes_ceiling.get() + 1,
                    )
                    .unwrap();
                    let refused = page.try_project::<Vec<u64>>(excessive, |_, _| {
                        called.store(true, Ordering::SeqCst);
                        panic!("output refusal must precede conversion")
                    });
                    assert!(matches!(
                        refused,
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    assert!(!called.load(Ordering::SeqCst));
                    assert_eq!(f.storage.status().read_bytes, input_charge);

                    let failed = page.try_project(
                        NonZeroUsize::new(4096).unwrap(),
                        |rows, allowance| -> Result<Vec<u64>> {
                            assert_eq!(rows.as_ptr(), original);
                            assert_eq!(rows[0].position, first);
                            assert!(
                                f.storage.status().read_bytes >= input_charge + allowance.get()
                            );
                            Err(Error::InvalidInput {
                                field: "fixture page conversion",
                                reason: "retry original rows",
                            })
                        },
                    );
                    assert!(failed.is_err());
                    assert_eq!(f.storage.status().read_bytes, input_charge);
                    let oversized = page
                        .try_project(NonZeroUsize::new(64).unwrap(), |_, allowance| {
                            Ok(Vec::<u8>::with_capacity(allowance.get() + 1))
                        });
                    assert!(matches!(
                        oversized,
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, input_charge);
                    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        page.try_project::<Vec<u64>>(NonZeroUsize::new(4096).unwrap(), |_, _| {
                            panic!("conversion panic")
                        })
                    }));
                    assert!(panicked.is_err());
                    assert_eq!(f.storage.status().read_bytes, input_charge);
                    assert_eq!(page.as_ptr(), original);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), views);

                    let projected = page
                        .try_project(NonZeroUsize::new(4096).unwrap(), |rows, _| {
                            Ok(rows.iter().map(|row| row.payload).collect::<Vec<_>>())
                        })
                        .unwrap();
                    assert_eq!(&*projected, &[7]);
                    let next = reader.next_page().await.unwrap().unwrap();
                    assert_eq!(next[0].position, second);
                    assert_eq!(next[0].payload, 8);
                    assert!(reader.next_page().await.unwrap().is_none());
                    drop((page, next, reader));
                    assert_eq!(
                        &*projected,
                        &[7],
                        "projection retains its own charge after input release"
                    );
                    assert!(f.storage.status().read_bytes >= projected.memory_size());
                    drop(projected);
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("overrun_uses_budget", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(9).await;
                    f.storage.flush().await.unwrap();
                    let mut query = RecordQuery::new(schema(), decode);
                    query.options.execution = Some(QueryLimits {
                        timeout: Duration::from_millis(100),
                        ..StorageSettings::default().query_execution
                    });
                    let mut reader = f.storage.query(query).await.unwrap();
                    reader.retain_view().unwrap();
                    let view = reader.view().unwrap().clone();
                    let mut sibling_query = RecordQuery::new(schema(), decode);
                    sibling_query.options.execution = view.execution_limits().unwrap();
                    let mut sibling = f.storage.query_in(&view, sibling_query).unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let input = page
                        .try_into_value(NonZeroUsize::new(64).unwrap(), |records, _| {
                            Ok(records[0].payload)
                        })
                        .unwrap();
                    assert!(reader.next_page().await.unwrap().is_none());
                    let retained = f.storage.status().read_bytes;
                    let calls = AtomicUsize::new(0);
                    let result =
                        reader.project(&input, NonZeroUsize::new(64).unwrap(), |value, _| {
                            calls.fetch_add(1, Ordering::SeqCst);
                            let mut nested = RecordQuery::new(schema(), decode);
                            nested.options.execution = view.execution_limits().unwrap();
                            assert!(matches!(
                                f.storage.query_in(&view, nested),
                                Err(Error::NotReady {
                                    domain: "saved view computation"
                                })
                            ));
                            std::thread::sleep(Duration::from_millis(120));
                            Ok(*value)
                        });
                    assert!(matches!(
                        result,
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    assert_eq!(*input, 9);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let retry =
                        reader.project(&input, NonZeroUsize::new(64).unwrap(), |value, _| {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Ok(*value)
                        });
                    assert!(matches!(
                        retry,
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    assert!(matches!(
                        sibling.next_page().await,
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    let mut reset = RecordQuery::new(schema(), decode);
                    reset.options.execution = view.execution_limits().unwrap();
                    assert!(matches!(
                        f.storage.query_in(&view, reset),
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    assert_eq!(*input, 9);
                    drop((input, reader, sibling, view));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("query_keeps_charge", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.append(9).await;
                    f.storage.flush().await.unwrap();
                    let mut reader = f
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let value = page
                        .try_into_value(NonZeroUsize::new(4096).unwrap(), |records, _| {
                            Ok(records
                                .into_iter()
                                .map(|record| record.payload)
                                .collect::<Vec<_>>())
                        })
                        .unwrap();
                    drop(reader);
                    let before = f.storage.status().read_bytes;
                    let shared = value.try_into_shared().unwrap();
                    let retained = f.storage.status().read_bytes;
                    assert!(retained > before);
                    let clone = shared.clone();
                    assert!(std::ptr::eq(&*shared, &*clone));
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let failed = shared.try_project(
                        NonZeroUsize::new(4096).unwrap(),
                        |values, _| -> Result<Vec<u64>> {
                            assert_eq!(values, &[9]);
                            assert!(f.storage.status().read_bytes >= retained + 4096);
                            Err(Error::InvalidInput {
                                field: "fixture borrowed value",
                                reason: "retry",
                            })
                        },
                    );
                    assert!(failed.is_err());
                    assert_eq!(f.storage.status().read_bytes, retained);
                    drop(shared);
                    assert_eq!(f.storage.status().read_bytes, retained);
                    let projected = clone
                        .try_project(NonZeroUsize::new(4096).unwrap(), |values, _| Ok(values[0]))
                        .unwrap();
                    assert_eq!(*projected, 9);
                    drop((clone, projected));
                    until(|| f.storage.status().read_bytes == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projection_retains_page", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let mut reader = f
                        .storage
                        .query(RecordQuery::new(schema(), decode))
                        .await
                        .unwrap();
                    reader.retain_view().unwrap();
                    let calls = AtomicUsize::new(0);
                    let failure = reader
                        .next_page_projected_including_empty(
                            NonZeroUsize::new(64).unwrap(),
                            |rows, _| -> Result<u64> {
                                assert!(rows.is_empty());
                                calls.fetch_add(1, Ordering::SeqCst);
                                Err(Error::InvalidInput {
                                    field: "fixture empty aggregate",
                                    reason: "retry",
                                })
                            },
                        )
                        .await;
                    assert!(matches!(
                        failure,
                        Err(Error::InvalidInput {
                            field: "fixture empty aggregate",
                            ..
                        })
                    ));
                    let retained = f.storage.status().read_bytes;
                    let reads = f.data.control.view_ids.lock().unwrap().len();
                    let output = reader
                        .next_page_projected_including_empty(
                            NonZeroUsize::new(64).unwrap(),
                            |rows, _| {
                                calls.fetch_add(1, Ordering::SeqCst);
                                Ok(rows.len() as u64)
                            },
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(*output, 0);
                    output.check_origin(&f.storage).unwrap();
                    assert_eq!(calls.load(Ordering::SeqCst), 2);
                    assert_eq!(f.data.control.view_ids.lock().unwrap().len(), reads);
                    assert!(
                        reader
                            .next_page_projected_including_empty(
                                NonZeroUsize::new(64).unwrap(),
                                |_, _| -> Result<u64> {
                                    panic!("exhausted source must not synthesize another page")
                                }
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    drop(output);
                    assert!(f.storage.status().read_bytes < retained);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
}
