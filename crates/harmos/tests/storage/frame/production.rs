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
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
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

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("work_retains_header", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let preparation = f
                        .storage
                        .prepare_read_production(NonZeroUsize::new(64).unwrap())
                        .unwrap();
                    let active = preparation.computation().unwrap();
                    let source = preparation
                        .finish(vec![6_u8; 32])
                        .unwrap()
                        .try_into_value()
                        .unwrap();
                    drop(active);
                    let original = source.as_ptr() as usize;
                    let input_charge = f.storage.status().read_bytes;
                    let preparation = source.prepare_work().unwrap();
                    let (release, waiting) = tokio::sync::oneshot::channel();
                    let (started, entered) = tokio::sync::oneshot::channel();
                    let future = async move {
                        started.send(()).unwrap();
                        waiting.await.unwrap();
                        assert_eq!(source.as_ptr() as usize, original);
                        source.try_project(NonZeroUsize::new(64).unwrap(), |bytes, _| {
                            Ok(bytes[..8].to_vec())
                        })
                    };
                    let work = preparation.boxed(future).unwrap();
                    let held = f.storage.status().read_bytes;
                    assert!(held > input_charge);
                    let task = tokio::spawn(work);
                    entered.await.unwrap();
                    drop(task);
                    assert_eq!(f.storage.status().read_bytes, held);
                    release.send(()).unwrap();
                    for _ in 0..100 {
                        tokio::task::yield_now().await;
                        if f.storage.status().read_bytes == baseline {
                            f.finish().await;
                            return;
                        }
                    }
                    panic!("accepted boxed work retained credit after disposal");
                });
        });
    }
    {
        scenario_runtime::check("work_refuses_oversize", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let settings = StorageSettings {
                        query_decoded_bytes: NonZeroUsize::new(128).unwrap(),
                        ..StorageSettings::default()
                    };
                    let f = Fixture::new(settings).await;
                    let view = f.storage.view_with_limits(f.execution).await.unwrap();
                    let baseline = f.storage.status().read_bytes;
                    let preparation = view.prepare_work().unwrap();
                    let bytes = [1_u8; 129];
                    let future = async move { std::hint::black_box(bytes) };
                    assert!(matches!(
                        preparation.boxed(future),
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    let active = view.begin_computation().unwrap();
                    let same = active.clone();
                    assert_eq!(active.deadline(), same.deadline());
                    drop(active);
                    assert!(matches!(
                        view.begin_computation(),
                        Err(Error::NotReady { .. })
                    ));
                    drop(same);
                    assert!(view.begin_computation().is_ok());
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("preparation_retains_credit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let preparation = f
                        .storage
                        .prepare_production(NonZeroUsize::new(4096).unwrap())
                        .unwrap();
                    let charged = f.storage.status().read_bytes;
                    assert!(charged >= baseline + preparation.allowance().get());
                    let (release, waiting) = tokio::sync::oneshot::channel::<()>();
                    let mut work = Box::pin(async move {
                        waiting.await.unwrap();
                        preparation.finish(vec![9_u8; 64])
                    });
                    assert!(
                        std::future::poll_fn(|context| {
                            use std::future::Future;
                            std::task::Poll::Ready(work.as_mut().poll(context).is_pending())
                        })
                        .await
                    );
                    assert_eq!(f.storage.status().read_bytes, charged);
                    release.send(()).unwrap();
                    let completed = work.await.unwrap();
                    assert_eq!(completed.value().as_slice(), [9_u8; 64]);
                    assert!(f.storage.status().read_bytes < charged);
                    drop(completed);
                    assert_eq!(f.storage.status().read_bytes, baseline);

                    let preparation = f
                        .storage
                        .prepare_production(NonZeroUsize::new(4096).unwrap())
                        .unwrap();
                    let (_release, waiting) = tokio::sync::oneshot::channel::<()>();
                    let mut cancelled = Box::pin(async move {
                        waiting.await.unwrap();
                        preparation.finish(vec![8_u8; 64])
                    });
                    assert!(
                        std::future::poll_fn(|context| {
                            use std::future::Future;
                            std::task::Poll::Ready(cancelled.as_mut().poll(context).is_pending())
                        })
                        .await
                    );
                    assert!(f.storage.status().read_bytes > baseline);
                    drop(cancelled);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("preparation_finishes_value", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let preparation = f
                        .storage
                        .prepare_production(NonZeroUsize::new(4096).unwrap())
                        .unwrap();
                    let output = vec![7_u8; 64];
                    let original = output.as_ptr();
                    let completed = preparation.finish(output).unwrap();
                    assert_eq!(completed.value().as_ptr(), original);
                    let charged = f.storage.status().read_bytes;
                    let shared = completed.try_into_value().unwrap();
                    assert_eq!(shared.as_ptr(), original);
                    assert_eq!(f.storage.status().read_bytes, charged);
                    drop(shared);
                    assert_eq!(f.storage.status().read_bytes, baseline);

                    let preparation = f
                        .storage
                        .prepare_production(NonZeroUsize::new(32).unwrap())
                        .unwrap();
                    assert!(matches!(
                        preparation.finish(vec![0_u8; 64]),
                        Err(Error::InvalidInput {
                            field: "storage production",
                            ..
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("preclaims_retains_actual", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let peak = Cell::new(0);
                    let produced = f
                        .storage
                        .produce(NonZeroUsize::new(4096).unwrap(), |allowance| {
                            peak.set(f.storage.status().read_bytes);
                            assert!(peak.get() >= baseline + allowance.get());
                            Ok(vec![7_u8; 64])
                        })
                        .unwrap();
                    assert_eq!(produced.value().as_slice(), [7_u8; 64]);
                    let retained = f.storage.status().read_bytes;
                    assert!(retained > baseline);
                    assert!(retained < peak.get());
                    drop(produced);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("production_refuses_another", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = StorageSettings {
                        read_memory_bytes: NonZeroUsize::new(4096).unwrap(),
                        ..StorageSettings::default()
                    };
                    settings.query_execution.memory_bytes = settings.read_memory_bytes;
                    settings.query_decoded_bytes = settings.read_memory_bytes;
                    let f = Fixture::new(settings).await;
                    let baseline = f.storage.status().read_bytes;
                    let produced = f
                        .storage
                        .produce(NonZeroUsize::new(2048).unwrap(), |_| Ok(vec![1_u8; 1800]))
                        .unwrap();
                    let held = f.storage.status().read_bytes;
                    let called = Cell::new(false);
                    let denied = f.storage.produce(NonZeroUsize::new(1024).unwrap(), |_| {
                        called.set(true);
                        Ok(vec![2_u8; 32])
                    });
                    assert!(matches!(denied, Err(Error::ResourceExhausted { .. })));
                    assert!(!called.get());
                    assert_eq!(f.storage.status().read_bytes, held);
                    drop(produced);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    let retry = f
                        .storage
                        .produce(NonZeroUsize::new(1024).unwrap(), |_| Ok(vec![2_u8; 32]))
                        .unwrap();
                    drop(retry);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("builders_release_credit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let oversized = f
                        .storage
                        .produce(NonZeroUsize::new(32).unwrap(), |_| Ok(vec![0_u8; 64]));
                    assert!(matches!(oversized, Err(Error::InvalidInput { .. })));
                    assert_eq!(f.storage.status().read_bytes, baseline);

                    let error =
                        f.storage
                            .produce::<Vec<u8>>(NonZeroUsize::new(1024).unwrap(), |_| {
                                Err(Error::InvalidInput {
                                    field: "producer",
                                    reason: "original failure",
                                })
                            });
                    assert!(matches!(
                        error,
                        Err(Error::InvalidInput {
                            field: "producer",
                            ..
                        })
                    ));
                    assert_eq!(f.storage.status().read_bytes, baseline);

                    let panic = catch_unwind(AssertUnwindSafe(|| {
                        f.storage
                            .produce::<Vec<u8>>(NonZeroUsize::new(1024).unwrap(), |_| {
                                panic!("producer panic")
                            })
                    }));
                    assert!(panic.is_err());
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("overflow_refuses_producer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let called = Cell::new(false);
                    let result = f
                        .storage
                        .produce(NonZeroUsize::new(usize::MAX).unwrap(), |_| {
                            called.set(true);
                            Ok(())
                        });
                    assert!(matches!(result, Err(Error::InvalidInput { .. })));
                    assert!(!called.get());
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("output_share_ledger", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let view = f
                        .storage
                        .view_with_limits(StorageSettings::default().query_execution)
                        .await
                        .unwrap();
                    let saved = view.saved().unwrap();
                    let input_charge = f.storage.status().read_bytes;
                    assert!(input_charge > 0);
                    let produced = f
                        .storage
                        .produce(NonZeroUsize::new(1024).unwrap(), |_| {
                            assert_eq!(view.saved().unwrap(), saved);
                            assert!(f.storage.status().read_bytes > input_charge);
                            Ok(saved.sequence.to_le_bytes().to_vec())
                        })
                        .unwrap();
                    assert_eq!(produced.value().as_slice(), saved.sequence.to_le_bytes());
                    assert!(f.storage.status().read_bytes > input_charge);
                    drop(produced);
                    assert_eq!(f.storage.status().read_bytes, input_charge);
                    assert_eq!(view.saved().unwrap(), saved);
                    drop(view);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("wait_keeps_output", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let produced = f
                        .storage
                        .produce(NonZeroUsize::new(1024).unwrap(), |_| Ok(vec![3_u8; 64]))
                        .unwrap();
                    let held = f.storage.status().read_bytes;
                    let (release, released) = std::sync::mpsc::channel();
                    let (ready, started) = tokio::sync::oneshot::channel();
                    let mut work = tokio::task::spawn_blocking(move || {
                        ready.send(()).unwrap();
                        released.recv().unwrap();
                        assert_eq!(produced.value().as_slice(), [3_u8; 64]);
                        drop(produced);
                    });
                    tokio::time::timeout(Duration::from_secs(1), started)
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(
                        tokio::time::timeout(Duration::from_millis(5), &mut work)
                            .await
                            .is_err()
                    );
                    assert_eq!(f.storage.status().read_bytes, held);
                    release.send(()).unwrap();
                    tokio::time::timeout(Duration::from_secs(1), &mut work)
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("output_leaves_destination", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let pending = f.storage.status().pending_bytes;
                    let produced = f
                        .storage
                        .produce(NonZeroUsize::new(8192).unwrap(), |_| Ok(vec![4_u8; 4096]))
                        .unwrap();
                    let held = f.storage.status().read_bytes;
                    assert_eq!(f.storage.status().pending_bytes, pending);
                    let metadata = FileMetadata {
                        id: Identifier::new("produced-report").unwrap(),
                        scope: None,
                        media_type: "application/octet-stream".to_owned(),
                        expected_length: Some(produced.value().len() as u64),
                        expected_sha256: None,
                    };
                    let file = tokio::time::timeout(
                        Duration::from_secs(1),
                        f.storage.write_file(metadata, produced.value()),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert_eq!(file.length, 4096);
                    assert!(f.storage.status().read_bytes >= held);
                    drop(produced);
                    drop(file);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("storage_keeps_existing", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let produced = f
                        .storage
                        .produce(NonZeroUsize::new(1024).unwrap(), |_| Ok(vec![5_u8; 64]))
                        .unwrap();
                    let held = f.storage.status().read_bytes;
                    f.storage.close().await.unwrap();
                    assert_eq!(produced.value().as_slice(), [5_u8; 64]);
                    assert_eq!(f.storage.status().read_bytes, held);
                    let called = Cell::new(false);
                    let result = f.storage.produce(NonZeroUsize::new(1024).unwrap(), |_| {
                        called.set(true);
                        Ok(())
                    });
                    assert!(matches!(result, Err(Error::Closing { domain: "storage" })));
                    assert!(!called.get());
                    drop(produced);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("projections_share_profile", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = StorageSettings::default();
                    settings.query_execution.timeout = Duration::from_secs(10);
                    settings.query_execution.memory_bytes = NonZeroUsize::new(4096).unwrap();
                    settings.query_decoded_bytes = NonZeroUsize::new(128).unwrap();
                    let f = Fixture::new(settings).await;
                    let baseline = f.storage.status().read_bytes;
                    let preparation = f
                        .storage
                        .prepare_read_production(NonZeroUsize::new(64).unwrap())
                        .unwrap();
                    assert_eq!(
                        preparation.execution_limits().unwrap().memory_bytes.get(),
                        4096
                    );
                    let acquisition = preparation.computation().unwrap();
                    let deadline = acquisition.deadline();
                    let source = preparation
                        .finish(vec![7_u8; 32])
                        .unwrap()
                        .try_into_value()
                        .unwrap()
                        .try_into_shared()
                        .unwrap();
                    let original = source.as_ptr();
                    tokio::time::advance(Duration::from_secs(3)).await;
                    assert_eq!(acquisition.deadline(), deadline);
                    assert_eq!(
                        deadline - tokio::time::Instant::now(),
                        Duration::from_secs(7)
                    );
                    acquisition.check().unwrap();
                    drop(acquisition);
                    // Time Between Calls Does Not Reset Or Spend The Original Allowance.
                    tokio::time::advance(Duration::from_secs(20)).await;
                    let work = source.computation().unwrap();
                    assert_eq!(work.timeout(), Duration::from_secs(7));
                    assert!(matches!(
                        source.clone().computation(),
                        Err(Error::NotReady { .. })
                    ));
                    let called = Cell::new(false);
                    assert!(
                        work.project_shared(&source, NonZeroUsize::new(129).unwrap(), |_, _| {
                            called.set(true);
                            Ok(Vec::<u8>::new())
                        })
                        .is_err()
                    );
                    assert!(!called.get());
                    let output = work
                        .project_shared(&source, NonZeroUsize::new(64).unwrap(), |input, _| {
                            assert_eq!(input.as_ptr(), original);
                            Ok(input[..8].to_vec())
                        })
                        .unwrap();
                    tokio::time::advance(Duration::from_secs(8)).await;
                    assert!(matches!(work.check(), Err(Error::ResourceExhausted { .. })));
                    drop(work);
                    drop(source);
                    assert!(matches!(
                        output.computation(),
                        Err(Error::ResourceExhausted { .. })
                    ));
                    assert!(f.storage.status().read_bytes > baseline);
                    drop(output);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_retains_owner", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut settings = StorageSettings::default();
                    settings.query_execution.timeout = Duration::from_secs(10);
                    let f = Fixture::new(settings).await;
                    let baseline = f.storage.status().read_bytes;
                    let preparation = f
                        .storage
                        .prepare_read_production(NonZeroUsize::new(64).unwrap())
                        .unwrap();
                    let opening = preparation.computation().unwrap();
                    let source = preparation
                        .finish(vec![5_u8; 32])
                        .unwrap()
                        .try_into_value()
                        .unwrap()
                        .try_into_shared()
                        .unwrap();
                    drop(opening);
                    let active = source.computation().unwrap();
                    let worker_source = source.clone();
                    let (release, released) = tokio::sync::oneshot::channel::<()>();
                    let mut accepted = tokio::spawn(async move {
                        released.await.unwrap();
                        active.project_shared(
                            &worker_source,
                            NonZeroUsize::new(64).unwrap(),
                            |input, _| Ok(input[..8].to_vec()),
                        )
                    });
                    let held = f.storage.status().read_bytes;
                    assert!(
                        tokio::time::timeout(Duration::from_secs(1), &mut accepted)
                            .await
                            .is_err()
                    );
                    assert!(matches!(source.computation(), Err(Error::NotReady { .. })));
                    assert_eq!(f.storage.status().read_bytes, held);
                    drop(source);
                    assert_eq!(f.storage.status().read_bytes, held);
                    release.send(()).unwrap();
                    let output = accepted.await.unwrap().unwrap();
                    assert!(f.storage.status().read_bytes > baseline);
                    let retained = output.computation().unwrap();
                    assert_eq!(retained.timeout(), Duration::from_secs(9));
                    drop(retained);
                    drop(output);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("guard_rejects_another", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let first = f
                        .storage
                        .prepare_read_production(NonZeroUsize::new(64).unwrap())
                        .unwrap();
                    let guard = first.computation().unwrap();
                    let first = first
                        .finish(vec![1_u8; 32])
                        .unwrap()
                        .try_into_value()
                        .unwrap();
                    let second = f
                        .storage
                        .prepare_read_production(NonZeroUsize::new(64).unwrap())
                        .unwrap();
                    let second_guard = second.computation().unwrap();
                    let second = second
                        .finish(vec![2_u8; 32])
                        .unwrap()
                        .try_into_value()
                        .unwrap();
                    drop(second_guard);
                    let called = Cell::new(false);
                    assert!(matches!(
                        guard.project(&second, NonZeroUsize::new(64).unwrap(), |_, _| {
                            called.set(true);
                            Ok(Vec::<u8>::new())
                        }),
                        Err(Error::InvalidInput {
                            field: "storage production computation",
                            ..
                        })
                    ));
                    assert!(!called.get());
                    assert_eq!(first.as_slice(), [1_u8; 32]);
                    drop((guard, first, second));
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
}

/* === Tests === */
