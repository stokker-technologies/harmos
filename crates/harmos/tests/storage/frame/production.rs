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
#[cfg(feature = "grpc")]
#[harmos::message]
struct Delivery {
    bytes: Vec<u8>,
}

struct SendOnly(Cell<u64>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl harmos::MemorySize for SendOnly {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn produced<T: harmos::MemorySize>(
    storage: &StorageManager,
    bytes: NonZeroUsize,
    value: T,
) -> QueryValue<T> {
    let preparation = storage.prepare_read_production(bytes).unwrap();
    let active = preparation.computation().unwrap();
    let value = preparation.finish(value).unwrap().try_into_value().unwrap();
    drop(active);
    value
}

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("response_metadata_refuses", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let probe = Fixture::new(StorageSettings::default()).await;
                    let value = produced(&probe.storage, NonZeroUsize::new(16).unwrap(), 7_u64);
                    let original = probe.storage.status().read_bytes;
                    let shared = value.try_into_shared().unwrap();
                    let shared_metadata = probe.storage.status().read_bytes - original;
                    drop(shared);
                    probe.finish().await;
                    let delivery = std::mem::size_of::<harmos::operations::Response<u64>>();
                    let delivery =
                        delivery.max(std::mem::size_of::<harmos::operations::StreamItem<u64>>());
                    let delivery_metadata =
                        shared_metadata + delivery - std::mem::size_of::<SharedQueryValue<u64>>();
                    assert!(delivery_metadata > shared_metadata);
                    for (ceiling, accepted) in [(shared_metadata, false), (delivery_metadata, true)]
                    {
                        let settings = StorageSettings {
                            query_decoded_bytes: NonZeroUsize::new(ceiling).unwrap(),
                            query_decoded_bytes_ceiling: NonZeroUsize::new(ceiling).unwrap(),
                            ..StorageSettings::default()
                        };
                        let f = Fixture::new(settings).await;
                        let baseline = f.storage.status().read_bytes;
                        let produce =
                            || produced(&f.storage, NonZeroUsize::new(16).unwrap(), 7_u64);
                        let shared = produce().try_into_shared().unwrap();
                        assert_eq!(*shared, 7);
                        drop(shared);
                        assert_eq!(f.storage.status().read_bytes, baseline);
                        let value = produce();
                        value.check_origin(&f.storage).unwrap();
                        let result = value.try_into_response();
                        if accepted {
                            let response = result.unwrap();
                            assert_eq!(*response, 7);
                            assert!(f.storage.status().read_bytes > baseline);
                            drop(response);
                        } else {
                            assert!(matches!(result, Err(Error::ResourceExhausted { .. })));
                        }
                        assert_eq!(f.storage.status().read_bytes, baseline);
                        f.finish().await;
                    }
                });
        });
    }
    {
        scenario_runtime::check("view_memory_budget", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(StorageSettings {
                        read_memory_bytes: NonZeroUsize::new(32 * 1024 * 1024).unwrap(),
                        ..StorageSettings::default()
                    })
                    .await;
                    let defaults = StorageSettings::default().query_execution;
                    let budget = QueryLimits {
                        memory_bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
                        ..defaults
                    };
                    // The Output Exceeds The Default Working Memory But Fits The View Budget.
                    let output = NonZeroUsize::new(6 * 1024 * 1024).unwrap();
                    assert!(matches!(
                        f.storage.prepare_read_production(output),
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    let interactive = f.storage.view_with_limits(defaults).await.unwrap();
                    assert!(matches!(
                        f.storage.prepare_read_production_in(&interactive, output),
                        Err(Error::ResourceExhausted {
                            resource: "query decoded memory"
                        })
                    ));
                    drop(interactive);
                    let finalization = f.storage.view_with_limits(budget).await.unwrap();
                    let baseline = f.storage.status().read_bytes;
                    let preparation = f
                        .storage
                        .prepare_read_production_in(&finalization, output)
                        .unwrap();
                    assert_eq!(preparation.execution_limits(), Some(budget));
                    assert!(f.storage.status().read_bytes >= baseline + budget.memory_bytes.get());
                    drop(preparation);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    drop(finalization);
                    f.finish().await;
                });
        });
    }
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

#[cfg(feature = "grpc")]
#[test]
fn response_delivery_contract() {
    use harmos::grpc::{Codec, DecodeSettings, EncodeSettings, Event};
    use harmos::operations::{Response, StreamItem};
    use http_body_util::BodyExt;
    use tonic::codec::Codec as _;

    for (name, cached, refused, cancelled, completion, streamed) in [
        (
            "response_encoding_releases",
            false,
            false,
            false,
            false,
            false,
        ),
        ("response_cache_retains", true, false, false, false, false),
        (
            "response_encoding_refuses",
            false,
            true,
            false,
            false,
            false,
        ),
        ("response_cancel_releases", false, false, true, false, false),
        (
            "response_completion_releases",
            false,
            false,
            false,
            true,
            false,
        ),
        (
            "owned_stream_encoding_releases",
            false,
            false,
            false,
            false,
            true,
        ),
        (
            "owned_stream_cache_retains",
            true,
            false,
            false,
            false,
            true,
        ),
        (
            "owned_stream_encoding_refuses",
            false,
            true,
            false,
            false,
            true,
        ),
        (
            "owned_stream_cancel_releases",
            false,
            false,
            true,
            false,
            true,
        ),
    ] {
        scenario_runtime::check(name, || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let value = produced(
                        &f.storage,
                        NonZeroUsize::new(1024).unwrap(),
                        Delivery { bytes: vec![7; 64] },
                    );
                    value.check_origin(&f.storage).unwrap();
                    let pointer = value.bytes.as_ptr();
                    let response = value.try_into_response().unwrap();
                    assert_eq!(response.bytes.as_ptr(), pointer);
                    let retained = f.storage.status().read_bytes;
                    let cache = cached.then(|| response.clone().into_erased());
                    let payload = harmos::Protobuf::encode_to_vec(&*response);
                    let limit = NonZeroUsize::new(if refused { 1 } else { 1024 }).unwrap();
                    let decode = DecodeSettings {
                        message_bytes: NonZeroUsize::new(1024).unwrap(),
                        retained_bytes: NonZeroUsize::new(4096).unwrap(),
                    };
                    let encode = EncodeSettings {
                        message_bytes: limit,
                        buffer_bytes: limit,
                    };
                    let mut body: std::pin::Pin<
                        Box<dyn http_body::Body<Data = bytes::Bytes, Error = tonic::Status> + Send>,
                    > = if streamed {
                        let (output, mut input) = harmos::operations::stream::<Delivery>(
                            harmos::operations::StreamSettings {
                                items: NonZeroUsize::new(1).unwrap(),
                                bytes: NonZeroUsize::new(2048).unwrap(),
                            },
                        )
                        .unwrap();
                        output.send_owned(response).await.unwrap();
                        assert_eq!(f.storage.status().read_bytes, retained);
                        let item = input.recv().await.unwrap();
                        assert_eq!(item.bytes.as_ptr(), pointer);
                        drop((input, output));
                        let mut codec = Codec::<
                            Delivery,
                            Event<StreamItem<Delivery>, Response<Delivery>>,
                        >::new(decode, encode)
                        .unwrap();
                        Box::pin(tonic::codec::EncodeBody::new_client(
                            codec.encoder(),
                            futures_util::stream::once(async move {
                                if cancelled {
                                    std::future::pending::<()>().await;
                                }
                                Ok(Event::Item(item))
                            }),
                            None,
                            None,
                        ))
                    } else if completion {
                        let mut codec = Codec::<
                            Delivery,
                            Event<StreamItem<Delivery>, Response<Delivery>>,
                        >::new(decode, encode)
                        .unwrap();
                        Box::pin(tonic::codec::EncodeBody::new_client(
                            codec.encoder(),
                            futures_util::stream::once(
                                async move { Ok(Event::Completion(response)) },
                            ),
                            None,
                            None,
                        ))
                    } else {
                        let mut codec =
                            Codec::<Delivery, Response<Delivery>>::new(decode, encode).unwrap();
                        Box::pin(tonic::codec::EncodeBody::new_client(
                            codec.encoder(),
                            futures_util::stream::once(async move {
                                if cancelled {
                                    std::future::pending::<()>().await;
                                }
                                Ok(response)
                            }),
                            None,
                            None,
                        ))
                    };
                    assert_eq!(f.storage.status().read_bytes, retained);
                    if cancelled {
                        use futures_util::FutureExt;
                        assert!(body.frame().now_or_never().is_none());
                        assert_eq!(f.storage.status().read_bytes, retained);
                    } else {
                        let frame = body.frame().await.unwrap();
                        if refused {
                            let error = frame.unwrap_err();
                            // Tonic's Framing Body Wraps Encoder Refusals As Internal.
                            assert_eq!(error.code(), tonic::Code::Internal);
                            assert!(
                                error
                                    .message()
                                    .contains("gRPC encoded message exceeds wire ceiling")
                            );
                        } else {
                            let data = frame.unwrap().into_data().unwrap();
                            let expected = if completion || streamed {
                                let mut bytes = vec![
                                    if streamed { 10 } else { 18 },
                                    u8::try_from(payload.len()).unwrap(),
                                ];
                                bytes.extend_from_slice(&payload);
                                bytes
                            } else {
                                payload
                            };
                            assert_eq!(&data[5..], &expected);
                        }
                        assert_eq!(
                            f.storage.status().read_bytes,
                            if cached { retained } else { baseline }
                        );
                    }
                    drop(body);
                    assert_eq!(
                        f.storage.status().read_bytes,
                        if cached { retained } else { baseline }
                    );
                    if let Some(cache) = &cache {
                        let restored = cache.clone().restore::<Delivery>().unwrap();
                        assert_eq!(restored.bytes.as_ptr(), pointer);
                    }
                    drop(cache);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
}

#[test]
fn owned_stream_admission() {
    use futures_util::FutureExt;
    use harmos::operations::{OutputEndpoint, StreamSettings, stream};

    for (name, mode) in [
        ("owned_stream_receiver_drop", 0),
        ("owned_stream_closed_send", 1),
        ("owned_stream_capacity_refusal", 2),
        ("owned_stream_pending_cancel", 3),
        ("owned_stream_budget_relay", 4),
    ] {
        scenario_runtime::check(name, || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let f = Fixture::new(StorageSettings::default()).await;
                    let baseline = f.storage.status().read_bytes;
                    let produce = || {
                        produced(&f.storage, NonZeroUsize::new(1024).unwrap(), vec![7_u8; 64])
                            .try_into_response()
                            .unwrap()
                    };
                    let response = produce();
                    let pointer = response.as_ptr();
                    let retained = f.storage.status().read_bytes;
                    let settings = StreamSettings {
                        items: NonZeroUsize::new(1).unwrap(),
                        bytes: NonZeroUsize::new(if mode == 2 { 1 } else { 2048 }).unwrap(),
                    };
                    let (output, mut input) = stream::<Vec<u8>>(settings).unwrap();
                    if mode == 1 {
                        input.close();
                    }
                    let sent = output.send_owned(response).await;
                    if mode == 1 || mode == 2 {
                        assert!(matches!(
                            sent,
                            Err(Error::NotReady { .. } | Error::ResourceExhausted { .. })
                        ));
                        assert!(!output.transferred());
                        assert_eq!(f.storage.status().read_bytes, baseline);
                    } else {
                        sent.unwrap();
                        assert!(output.transferred());
                        assert_eq!(f.storage.status().read_bytes, retained);
                        if mode == 3 {
                            let second = produce();
                            assert!(f.storage.status().read_bytes > retained);
                            let mut sending = Box::pin(output.send_owned(second));
                            assert!(sending.as_mut().now_or_never().is_none());
                            assert!(f.storage.status().read_bytes > retained);
                            drop(sending);
                            assert_eq!(f.storage.status().read_bytes, retained);
                        }
                        if mode == 4 {
                            let item = input.recv().await.unwrap();
                            let (relay, mut received) = stream::<Vec<u8>>(settings).unwrap();
                            relay.send_item(item).await.unwrap();
                            assert_eq!(f.storage.status().read_bytes, retained);
                            let delivered = received.recv().await.unwrap();
                            assert_eq!(delivered.as_ptr(), pointer);
                            assert!(matches!(
                                delivered.into_request(),
                                Err(harmos_contract::operations::streaming::Error::Ownership)
                            ));
                            assert_eq!(f.storage.status().read_bytes, baseline);
                        }
                    }
                    drop(input);
                    assert_eq!(f.storage.status().read_bytes, baseline);
                    f.finish().await;
                });
        });
    }
    scenario_runtime::check("ordinary_stream_send_only", || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (output, mut input) = stream::<SendOnly>(StreamSettings {
                    items: NonZeroUsize::new(1).unwrap(),
                    bytes: NonZeroUsize::new(64).unwrap(),
                })
                .unwrap();
                let receiver = tokio::spawn(async move {
                    input
                        .recv()
                        .await
                        .unwrap()
                        .into_request()
                        .unwrap()
                        .0
                        .0
                        .get()
                });
                output
                    .send_with(std::mem::size_of::<SendOnly>(), || {
                        Ok(SendOnly(Cell::new(7)))
                    })
                    .await
                    .unwrap();
                assert_eq!(receiver.await.unwrap(), 7);
            });
    });
}
