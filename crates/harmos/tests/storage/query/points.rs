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

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("value_retains_charge", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let value = f
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
                    assert!(f.storage.status().read_bytes >= value.memory_size());
                    assert_eq!(value.value.value, 1);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    drop(value);
                    assert_eq!(f.storage.status().read_bytes, 0);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reads_preserve_fixed", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let first = f
                        .storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    f.storage.flush_until(&first.position).await.unwrap();
                    let view = f
                        .storage
                        .view_with_limits(storage_settings().query_execution)
                        .await
                        .unwrap();
                    let pinned = f.storage.status().read_bytes;
                    let second = f
                        .storage
                        .write_object(&Document { id: 7, value: 2 })
                        .await
                        .unwrap();
                    f.storage.flush_until(&second.position).await.unwrap();

                    let value = f
                        .storage
                        .read_object_in::<Document>(
                            &view,
                            &7,
                            ObjectVersion::Latest,
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(value.position, first.position);
                    assert_eq!(value.version, first.version);
                    assert_eq!(value.value.value, 1);
                    assert!(f.storage.status().read_bytes >= pinned + value.memory_size());
                    let charged = f.storage.status().read_bytes;
                    assert!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &7,
                                ObjectVersion::Exact(second.version.clone()),
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await
                            .unwrap()
                            .is_none()
                    );
                    assert_eq!(f.storage.status().read_bytes, charged);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    drop(value);
                    assert_eq!(f.storage.status().read_bytes, pinned);
                    drop(view);
                    until(|| f.storage.status().read_bytes == 0).await;

                    let current = f
                        .storage
                        .read_object::<Document>(
                            &7,
                            ObjectVersion::Latest,
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(current.position, second.position);
                    assert_eq!(current.version, second.version);
                    assert_eq!(current.value.value, 2);
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 2);
                    drop(current);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("reads_refuse_limits", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    let other = Fixture::new().await;
                    let execution = storage_settings().query_execution;
                    let view = f.storage.view_with_limits(execution).await.unwrap();
                    let different = f
                        .storage
                        .view_with_limits(QueryLimits {
                            timeout: execution.timeout / 2,
                            ..execution
                        })
                        .await
                        .unwrap();
                    let pinned = f.storage.status().read_bytes;
                    assert!(matches!(
                        other
                            .storage
                            .read_object_in::<Document>(
                                &view,
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await,
                        Err(Error::InvalidInput {
                            field: "query view",
                            reason: "view belongs to another manager"
                        })
                    ));
                    // A View With Its Own Budget Reads Under That Budget.
                    drop(
                        f.storage
                            .read_object_in::<Document>(
                                &different,
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(4096).unwrap(),
                            )
                            .await
                            .unwrap(),
                    );
                    assert_eq!(f.storage.status().read_bytes, pinned);
                    assert_eq!(other.storage.status().read_bytes, 0);
                    f.storage.close().await.unwrap();
                    assert!(matches!(
                        f.storage
                            .read_object_in::<Document>(
                                &view,
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await,
                        Err(Error::ViewExpired)
                    ));
                    drop(view);
                    drop(different);
                    f.finish().await;
                    other.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("point_timeout_releases", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::with_settings(StorageSettings {
                        query_execution: QueryLimits {
                            timeout: Duration::from_millis(50),
                            ..storage_settings().query_execution
                        },
                        ..storage_settings()
                    })
                    .await;
                    f.storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let storage = f.storage.clone();
                    let reading = tokio::spawn(async move {
                        storage
                            .read_object::<Document>(
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(4096).unwrap(),
                            )
                            .await
                    });
                    f.data.control.read_entered.notified().await;
                    assert!(matches!(
                        reading.await.unwrap(),
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    until(|| f.storage.status().read_bytes == 0).await;
                    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 1);
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    let value = f
                        .storage
                        .read_object::<Document>(
                            &7,
                            ObjectVersion::Latest,
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(value.value.value, 1);
                    drop(value);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("view_budget_outlasts_default", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let defaults = QueryLimits {
                        timeout: Duration::from_millis(50),
                        ..storage_settings().query_execution
                    };
                    let f = Fixture::with_settings(StorageSettings {
                        query_execution: defaults,
                        ..storage_settings()
                    })
                    .await;
                    f.storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    let budget = QueryLimits {
                        timeout: Duration::from_secs(10),
                        ..defaults
                    };
                    let interactive = f.storage.view_with_limits(defaults).await.unwrap();
                    let finalization = f.storage.view_with_limits(budget).await.unwrap();
                    assert_eq!(finalization.execution_limits().unwrap(), Some(budget));
                    // The Backend Holds Each Read Well Past The 50 ms Default Budget.
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    let read = |view: StorageView| {
                        let storage = f.storage.clone();
                        tokio::spawn(async move {
                            storage
                                .read_object_in::<Document>(
                                    &view,
                                    &7,
                                    ObjectVersion::Latest,
                                    NonZeroUsize::new(4096).unwrap(),
                                )
                                .await
                                .map(|value| value.map(|value| value.value.value))
                        })
                    };
                    let reading = read(interactive.clone());
                    f.data.control.read_entered.notified().await;
                    assert!(matches!(
                        reading.await.unwrap(),
                        Err(Error::ResourceExhausted {
                            resource: "query computation time"
                        })
                    ));
                    let reading = read(finalization.clone());
                    f.data.control.read_entered.notified().await;
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    f.data.control.read_release.notify_one();
                    assert_eq!(reading.await.unwrap().unwrap(), Some(1));
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    // Queries Inherit The View Budget Unless They Name A Different One.
                    let key = 7;
                    let mut query = ObjectQuery::<Document>::new();
                    query.item = Some(&key);
                    let mut result = f.storage.query_in(&finalization, query).unwrap();
                    let page = result.next_page().await.unwrap().unwrap();
                    assert_eq!(page[0].value.value, 1);
                    drop((page, result));
                    let mut conflicting = ObjectQuery::<Document>::new();
                    conflicting.item = Some(&key);
                    conflicting.options.execution = Some(defaults);
                    assert!(matches!(
                        f.storage.query_in(&finalization, conflicting),
                        Err(Error::InvalidInput {
                            field: "query view",
                            reason: "view was not acquired with the same execution limits"
                        })
                    ));
                    drop((interactive, finalization));
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("point_page_ceiling", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new().await;
                    f.data.control.block_view.store(true, Ordering::SeqCst);
                    let before = f.storage.status().read_bytes;
                    assert!(
                        f.storage
                            .read_object::<Document>(
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(8193).unwrap()
                            )
                            .await
                            .is_err()
                    );
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 0);
                    assert_eq!(f.storage.status().read_bytes, before);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("working_bound_enforced", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::with_settings(StorageSettings {
                        query_execution: QueryLimits {
                            memory_bytes: NonZeroUsize::new(1).unwrap(),
                            ..storage_settings().query_execution
                        },
                        ..storage_settings()
                    })
                    .await;
                    f.storage
                        .write_object(&Document { id: 7, value: 1 })
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    assert!(
                        f.storage
                            .read_object::<Document>(
                                &7,
                                ObjectVersion::Latest,
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await
                            .is_err()
                    );
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
                    until(|| f.storage.status().read_bytes == 0).await;
                    assert_eq!(f.data.control.releases.load(Ordering::SeqCst), 0);
                    f.finish().await;
                });
        });
    }
}
