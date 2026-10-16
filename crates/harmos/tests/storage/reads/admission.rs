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
const MIB: usize = 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn page() -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(1).unwrap(),
        bytes: NonZeroUsize::new(1024).unwrap(),
    }
}

fn ceiling(f: &Fixture, bytes: usize) -> QueryLimits {
    QueryLimits {
        memory_bytes: NonZeroUsize::new(bytes).unwrap(),
        ..f.execution
    }
}

fn pool(f: &Fixture) -> usize {
    f.storage.status().read_bytes
}

async fn until(mut ready: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while !ready() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

async fn first_position(storage: &StorageManager, view: &StorageView) -> u64 {
    let mut reader = storage.query_in(view, record_query(page())).unwrap();
    let page = reader.next_page().await.unwrap().unwrap();
    page[0].position.sequence
}

async fn saved(f: &Fixture) {
    write(&f.storage, "saved").await;
    f.storage.flush().await.unwrap();
}

fn read_memory_refusal<T>(result: Result<T, Error>) -> bool {
    matches!(
        result,
        Err(Error::ResourceExhausted {
            resource: "storage read memory"
        })
    )
}

/* === Tests === */
#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("elastic_views_exceed_pool_together", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    saved(&f).await;
                    // Both Ceilings Together Exceed The 16 MiB Read Pool.
                    let limits = ceiling(&f, 12 * MIB).elastic();
                    let first = f.storage.view_with_limits(limits).await.unwrap();
                    let second = f.storage.view_with_limits(limits).await.unwrap();
                    assert!(pool(&f) < 12 * MIB);
                    assert_eq!(first_position(&f.storage, &first).await, 1);
                    assert_eq!(first_position(&f.storage, &second).await, 1);
                    assert_eq!(first.charged_bytes().unwrap(), 0);
                    assert_eq!(first.execution_limits().unwrap(), Some(limits));
                    drop((first, second));
                    until(|| pool(&f) == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("guaranteed_waits_where_elastic_fits", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    saved(&f).await;
                    let guaranteed = ceiling(&f, 12 * MIB);
                    assert_eq!(guaranteed.admission, Admission::Guaranteed);
                    let held = f.storage.view_with_limits(guaranteed).await.unwrap();
                    assert!(pool(&f) > 12 * MIB);
                    // A Second Guarantee Waits For Shared Capacity Instead Of Failing.
                    let mut waiting = Box::pin(f.storage.view_with_limits(guaranteed));
                    assert!(
                        std::future::poll_fn(|cx| std::task::Poll::Ready(
                            std::future::Future::poll(waiting.as_mut(), cx)
                        ))
                        .await
                        .is_pending()
                    );
                    let elastic = f
                        .storage
                        .view_with_limits(guaranteed.elastic())
                        .await
                        .unwrap();
                    assert_eq!(first_position(&f.storage, &elastic).await, 1);
                    drop((held, elastic));
                    let admitted = waiting.await.unwrap();
                    assert_eq!(admitted.execution_limits().unwrap(), Some(guaranteed));
                    drop(admitted);
                    until(|| pool(&f) == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("elastic_production_waits_for_guarantee", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    saved(&f).await;
                    let capacity = config().budget.read_memory_bytes.get();
                    let elastic = f
                        .storage
                        .view_with_limits(ceiling(&f, 4 * MIB).elastic())
                        .await
                        .unwrap();
                    let floor = pool(&f);
                    // Measure The View Metadata, Then Leave 1 MiB Of Pool Free.
                    let probe = f.storage.view_with_limits(ceiling(&f, MIB)).await.unwrap();
                    let metadata = pool(&f) - floor - MIB;
                    drop(probe);
                    until(|| pool(&f) == floor).await;
                    let reserved = capacity - floor - metadata - MIB;
                    let guaranteed = f
                        .storage
                        .view_with_limits(ceiling(&f, reserved))
                        .await
                        .unwrap();
                    assert_eq!(pool(&f), capacity - MIB);
                    // Elastic Output Larger Than The Free MiB Must Wait For The Guarantee.
                    let output = NonZeroUsize::new(2 * MIB).unwrap();
                    assert!(read_memory_refusal(
                        f.storage.prepare_read_production_in(&elastic, output)
                    ));
                    assert_eq!(elastic.charged_bytes().unwrap(), 0);
                    assert_eq!(pool(&f), capacity - MIB);
                    drop(guaranteed);
                    until(|| pool(&f) == floor).await;
                    let production = f
                        .storage
                        .prepare_read_production_in(&elastic, output)
                        .unwrap();
                    let charged = elastic.charged_bytes().unwrap();
                    assert!(charged >= 2 * MIB);
                    assert_eq!(pool(&f), floor + charged);
                    drop(production);
                    assert_eq!(elastic.charged_bytes().unwrap(), 0);
                    assert_eq!(pool(&f), floor);
                    drop(elastic);
                    until(|| pool(&f) == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("standalone_production_shares_guaranteed_pool", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let capacity = config().budget.read_memory_bytes.get();
                    let probe = f.storage.view_with_limits(ceiling(&f, MIB)).await.unwrap();
                    let metadata = pool(&f) - MIB;
                    drop(probe);
                    until(|| pool(&f) == 0).await;
                    // Leave One MiB Free; Plain Production Charges Its Requested Output.
                    let guaranteed = f
                        .storage
                        .view_with_limits(ceiling(&f, capacity - metadata - MIB))
                        .await
                        .unwrap();
                    let occupied = pool(&f);
                    assert_eq!(occupied, capacity - MIB);
                    let called = std::cell::Cell::new(false);
                    let small = f
                        .storage
                        .produce(NonZeroUsize::new(64).unwrap(), |_| Ok(vec![7_u8; 16]))
                        .unwrap();
                    drop(small);
                    assert_eq!(pool(&f), occupied);
                    assert!(read_memory_refusal(f.storage.produce(
                        NonZeroUsize::new(2 * MIB).unwrap(),
                        |_| {
                            called.set(true);
                            Ok(vec![7_u8; 16])
                        }
                    )));
                    assert!(!called.get(), "refusal precedes producer allocation");
                    assert_eq!(
                        pool(&f),
                        occupied,
                        "failed standalone reservation cannot leak"
                    );
                    drop(guaranteed);
                    until(|| pool(&f) == 0).await;
                    let output = f
                        .storage
                        .produce(NonZeroUsize::new(64).unwrap(), |_| Ok(vec![7_u8; 16]))
                        .unwrap()
                        .try_into_value()
                        .unwrap();
                    assert_eq!(&**output, &[7_u8; 16]);
                    assert!(pool(&f) > 0 && pool(&f) <= capacity);
                    drop(output);
                    until(|| pool(&f) == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("guaranteed_charges_draw_from_ceiling", || {
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
                    saved(&f).await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let held = pool(&f);
                    let mut reader = f.storage.query_in(&view, record_query(page())).unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let projected = page
                        .try_project(NonZeroUsize::new(MIB).unwrap(), |_, _| {
                            Ok(vec![0_u8; MIB / 2])
                        })
                        .unwrap();
                    // The Ceiling Reserved At Open Already Covers Both Charges.
                    assert!(view.charged_bytes().unwrap() > MIB / 2);
                    assert_eq!(pool(&f), held);
                    drop((page, reader));
                    // Output Outliving Its View Returns To The Pool With The Ceiling.
                    tokio::time::advance(Duration::from_millis(11)).await;
                    assert!(matches!(view.charged_bytes(), Err(Error::ViewExpired)));
                    until(|| pool(&f) < held).await;
                    assert!(pool(&f) > MIB / 2);
                    drop(projected);
                    until(|| pool(&f) == 0).await;
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("elastic_charges_return_after_drop", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    saved(&f).await;
                    let view = f
                        .storage
                        .view_with_limits(f.execution.elastic())
                        .await
                        .unwrap();
                    let floor = pool(&f);
                    let mut reader = f.storage.query_in(&view, record_query(page())).unwrap();
                    let page = reader.next_page().await.unwrap().unwrap();
                    let charged = view.charged_bytes().unwrap();
                    assert!(charged > 0);
                    assert_eq!(pool(&f), floor + charged);
                    drop(page);
                    drop(reader);
                    assert_eq!(view.charged_bytes().unwrap(), 0);
                    assert_eq!(pool(&f), floor);
                    drop(view);
                    until(|| pool(&f) == 0).await;
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("elastic_charges_return_after_idle_expiry", || {
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
                    saved(&f).await;
                    let view = f
                        .storage
                        .view_with_limits(f.execution.elastic())
                        .await
                        .unwrap();
                    let mut reader = f.storage.query_in(&view, record_query(page())).unwrap();
                    drop(reader.next_page().await.unwrap().unwrap());
                    assert_eq!(view.charged_bytes().unwrap(), 0);
                    assert!(pool(&f) > 0);
                    tokio::time::advance(Duration::from_millis(11)).await;
                    assert!(matches!(view.charged_bytes(), Err(Error::ViewExpired)));
                    until(|| pool(&f) == 0).await;
                    drop((reader, view));
                    f.finish().await;
                });
        });
    }
}
