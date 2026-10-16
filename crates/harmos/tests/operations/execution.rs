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

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("retry_retains_preparation", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_operation::<ComputeCall>("app", Lanes::Open, Arc::clone(&owner))
                        .unwrap();
                    let mut invocation = runtime
                        .operations
                        .submit_as::<Compute>(Request {
                            failures: 2,
                            ..request()
                        })
                        .unwrap();
                    let id = invocation.id();
                    let generation = invocation.generation();
                    let response = (&mut invocation).await.unwrap();
                    assert_eq!(invocation.id(), id);
                    assert_eq!(invocation.generation(), generation);
                    assert_eq!(invocation.status().attempt(), 2);
                    assert_eq!(
                        invocation.status().phase(),
                        harmos::operations::OperationPhase::Settled
                    );
                    assert_eq!(response.attempts, 3);
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(*owner.seen.lock().unwrap(), vec![id; 3]);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("errors_never_retry", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_operation::<ComputeCall>("app", Lanes::Open, Arc::clone(&owner))
                        .unwrap();
                    for failure in [
                        FailureKind::Permanent,
                        FailureKind::Unauthorized,
                        FailureKind::Invalid,
                        FailureKind::Cancelled,
                    ] {
                        assert!(matches!(
                            runtime
                                .operations
                                .call_as::<Compute>(Request {
                                    failures: 10,
                                    failure,
                                    ..request()
                                })
                                .await,
                            Err(Error::OperationFailed { .. })
                        ));
                    }
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 4);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("ambiguous_timeout_replay", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_operation::<SlowCall>("app", Lanes::Open, Arc::clone(&owner))
                        .unwrap();
                    assert!(matches!(
                        runtime.operations.call_as::<Slow>(request()).await,
                        Err(Error::OperationRefused(Refusal::TimedOut))
                    ));
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cache_reuses_owned", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_operation::<ComputeCall>("app", Lanes::Open, Arc::clone(&owner))
                        .unwrap();
                    let first = runtime
                        .operations
                        .call_as::<Compute>(Request {
                            keyed: true,
                            ..request()
                        })
                        .await
                        .unwrap();
                    let second = runtime
                        .operations
                        .call_as::<Compute>(Request {
                            keyed: true,
                            ..request()
                        })
                        .await
                        .unwrap();
                    assert!(first.ptr_eq(&second));
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
                    // A cached response is reused without constructing the operation.
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    assert!(matches!(
                        runtime
                            .operations
                            .call_as::<Compute>(Request {
                                keyed: true,
                                valid: false,
                                ..request()
                            })
                            .await,
                        Err(Error::OperationFailed { .. })
                    ));
                    runtime
                        .operations
                        .call_as::<Compute>(request())
                        .await
                        .unwrap();
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 2);
                    runtime.operations.set_cache_capacity(0);
                    runtime
                        .operations
                        .call_as::<Compute>(Request {
                            keyed: true,
                            ..request()
                        })
                        .await
                        .unwrap();
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 3);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("owner_never_retargets", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    runtime
                        .operations
                        .register_operation::<ComputeCall>("app", Lanes::Open, Arc::clone(&owner))
                        .unwrap();
                    let opened = Arc::new(Notify::new());
                    let gate = Arc::new(Notify::new());
                    let invocation = runtime
                        .operations
                        .submit_as::<Compute>(Request {
                            keyed: true,
                            gate: Some((Arc::clone(&opened), Arc::clone(&gate))),
                            ..request()
                        })
                        .unwrap();
                    opened.notified().await;
                    assert!(
                        tokio::time::timeout(
                            Duration::from_millis(10),
                            runtime.operations.unregister(Compute::ID)
                        )
                        .await
                        .is_err()
                    );
                    assert!(runtime.operations.definition(Compute::ID).is_err());
                    assert!(matches!(
                        runtime.operations.register_operation::<ComputeCall>(
                            "app",
                            Lanes::Open,
                            Arc::new(Owner::default())
                        ),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    gate.notify_one();
                    assert_eq!(invocation.await.unwrap().value, 7);
                    // Settled work does not release an interrupted explicit unregister's reservation.
                    assert!(matches!(
                        runtime.operations.register_operation::<ComputeCall>(
                            "app",
                            Lanes::Open,
                            Arc::new(Owner::default())
                        ),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    runtime.operations.unregister(Compute::ID).await.unwrap();
                    let replacement = Arc::new(Owner {
                        bias: 100,
                        ..Owner::default()
                    });
                    runtime
                        .operations
                        .register_operation::<ComputeCall>(
                            "app",
                            Lanes::Open,
                            Arc::clone(&replacement),
                        )
                        .unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Compute>(Request {
                                keyed: true,
                                ..request()
                            })
                            .await
                            .unwrap()
                            .value,
                        107
                    );
                    assert_eq!(replacement.runs.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("operations_use_structured", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = runtime().await;
    runtime
        .operations
        .register_operation::<ComputeCall>("app", Lanes::Open, Arc::new(Owner::default()))
        .unwrap();
    runtime
        .operations
        .register_operation::<ParentCall>("app", Lanes::Serial, Arc::new(ParentOwner))
        .unwrap();
    assert_eq!(
        runtime
            .operations
            .call_as::<Parent>(request())
            .await
            .unwrap()
            .value,
        7
    );
    runtime.operations.unregister(Compute::ID).await.unwrap();
    runtime
        .operations
        .register_operation::<ComputeCall>("app", Lanes::Serial, Arc::new(Owner::default()))
        .unwrap();
    assert!(
        matches!(runtime.operations.call_as::<Parent>(request()).await, Err(Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<Failed>(), Some(Failed::Nested(Error::OperationLaneCycle { .. }))))
    );
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("concurrent_ancestor_reentry", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    runtime
        .operations
        .register_operation::<ComputeCall>("app", Lanes::Concurrent, Arc::clone(&owner))
        .unwrap();
    runtime
        .operations
        .register_operation::<ParentCall>("app", Lanes::Concurrent, Arc::new(ParentOwner))
        .unwrap();

    assert!(
        matches!(runtime.operations.call_as::<Parent>(request()).await, Err(Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<Failed>(), Some(Failed::Nested(Error::OperationLaneCycle { .. }))))
    );
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);

    // An independent root still uses this bounded lane normally.
    assert_eq!(
        runtime
            .operations
            .call_as::<Compute>(request())
            .await
            .unwrap()
            .value,
        7
    );
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("shutdown_releases_bindings", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    let weak = Arc::downgrade(&owner);
                    runtime
                        .operations
                        .register_operation::<ComputeCall>("app", Lanes::Open, owner)
                        .unwrap();
                    runtime
                        .operations
                        .call_as::<Compute>(Request {
                            keyed: true,
                            ..request()
                        })
                        .await
                        .unwrap();
                    runtime.stop().await.unwrap();
                    assert!(weak.upgrade().is_none());
                    assert!(matches!(
                        runtime.operations.submit_as::<Compute>(request()),
                        Err(Error::Closing { .. })
                    ));
                });
        });
    }
    {
        scenario_runtime::check("handler_panic_settles", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    runtime
                        .operations
                        .register_operation::<ComputeCall>(
                            "app",
                            Lanes::Open,
                            Arc::new(Owner::default()),
                        )
                        .unwrap();
                    assert!(matches!(
                        runtime
                            .operations
                            .call_as::<Compute>(Request {
                                value: usize::MAX,
                                ..request()
                            })
                            .await,
                        Err(Error::OperationPanicked { .. })
                    ));
                    tokio::time::timeout(
                        Duration::from_secs(1),
                        runtime.operations.unregister(Compute::ID),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("operation_catalog_binding", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let owner = Arc::new(Owner::default());
                    let definitions = harmos::operations::Declarations::of::<Operations>().unwrap();
                    assert_eq!(definitions[0].id.as_str(), Compute::ID);
                    assert_eq!(definitions[1].id.as_str(), Slow::ID);
                    runtime
                        .operations
                        .register_operations::<Implemented, _>("app", Lanes::Open, owner.clone())
                        .unwrap();
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Compute>(request())
                            .await
                            .unwrap()
                            .value,
                        7
                    );

                    runtime.operations.unregister(Compute::ID).await.unwrap();
                    assert!(matches!(
                        runtime.operations.register_operations::<Implemented, _>(
                            "app",
                            Lanes::Open,
                            owner.clone()
                        ),
                        Err(Error::AlreadyRegistered { .. })
                    ));
                    assert!(runtime.operations.definition(Compute::ID).is_err());
                    assert!(runtime.operations.definition(Slow::ID).is_ok());
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("must_match_declarations", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    struct Mismatched;
    impl harmos::catalog::Catalog<harmos::operations::AuthoredDeclarations> for Mismatched {
        type Entries = [harmos::operations::Entry; 1];
        const ENTRIES: Self::Entries = [harmos::operations::Entry::of::<Slow>()];
    }
    impl harmos::catalog::Catalog<harmos::operations::Implementations<Owner>> for Mismatched {
        type Entries = [harmos::operations::Binding<Owner>; 1];
        const ENTRIES: Self::Entries =
            [
                <harmos::operations::Implementations<Owner> as harmos::catalog::Member<
                    ComputeCall,
                >>::ENTRY,
            ];
    }
    let runtime = runtime().await;
    let owner = Arc::new(Owner::default());
    assert!(matches!(
        runtime
            .operations
            .register_operations::<Mismatched, _>("app", Lanes::Open, owner.clone()),
        Err(Error::InvalidInput {
            field: "operation catalog",
            ..
        })
    ));
    assert!(runtime.operations.definitions().is_empty());
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("builder_binds_catalog", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let owner = Arc::new(Owner {
                        bias: 20,
                        ..Owner::default()
                    });
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .operations::<Implemented, _>("app", Lanes::Serial, Arc::clone(&owner))
                        .runtime()
                        .await
                        .unwrap();
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Compute>(request())
                            .await
                            .unwrap()
                            .value,
                        27
                    );
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("catalogs_refuse_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let owner = Arc::new(Owner::default());
                    let result = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .operations::<Implemented, _>("first", Lanes::Open, Arc::clone(&owner))
                        .operations::<Implemented, _>("first", Lanes::Open, Arc::clone(&owner))
                        .runtime()
                        .await;
                    let error = result.err().expect("duplicate catalogs refuse boot");
                    let cause = std::error::Error::source(&error).unwrap();
                    let source = std::error::Error::source(cause).unwrap();
                    assert!(matches!(
                        source.downcast_ref::<Error>(),
                        Some(Error::AlreadyRegistered { .. })
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
                    assert_eq!(Arc::strong_count(&owner), 1);
                });
        });
    }
    {
        scenario_runtime::check("providers_keep_cache", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let first = Arc::new(Owner {
                        bias: 10,
                        ..Owner::default()
                    });
                    let second = Arc::new(Owner {
                        bias: 20,
                        ..Owner::default()
                    });
                    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .register(Lanes::catalog())
                        .operations::<Implemented, _>("first", Lanes::Open, Arc::clone(&first))
                        .operations::<Implemented, _>("second", Lanes::Open, Arc::clone(&second))
                        .runtime()
                        .await
                        .unwrap();
                    assert_eq!(first.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(second.prepared.load(Ordering::SeqCst), 0);
                    assert!(matches!(
                        runtime.operations.submit_as::<Compute>(request()),
                        Err(Error::InvalidInput {
                            field: "operation provider",
                            ..
                        })
                    ));
                    assert!(matches!(
                        runtime.operations.unregister(Compute::ID).await,
                        Err(Error::InvalidInput {
                            field: "operation provider",
                            ..
                        })
                    ));
                    assert_eq!(
                        runtime
                            .operations
                            .definition_from("first", Compute::ID)
                            .unwrap()
                            .id
                            .as_str(),
                        Compute::ID
                    );
                    let keyed = || Request {
                        keyed: true,
                        ..request()
                    };
                    let first_result = runtime
                        .operations
                        .submit_from::<Compute>("first", keyed())
                        .unwrap()
                        .await
                        .unwrap();
                    let second_result = runtime
                        .operations
                        .submit_from::<Compute>("second", keyed())
                        .unwrap()
                        .await
                        .unwrap();
                    assert_eq!(first_result.value, 17);
                    assert_eq!(second_result.value, 27);
                    assert!(!first_result.ptr_eq(&second_result));
                    let again = runtime
                        .operations
                        .submit_from::<Compute>("second", keyed())
                        .unwrap()
                        .await
                        .unwrap();
                    assert!(second_result.ptr_eq(&again));
                    assert_eq!(second.runs.load(Ordering::SeqCst), 1);
                    let key = Compute::key(&keyed()).unwrap();
                    assert!(matches!(
                        runtime.operations.invalidate::<Compute>(&key),
                        Err(Error::InvalidInput {
                            field: "operation provider",
                            ..
                        })
                    ));
                    assert!(matches!(
                        runtime
                            .operations
                            .invalidate_from::<Compute>("missing", &key),
                        Err(Error::NotRegistered { .. })
                    ));
                    let unchanged = runtime
                        .operations
                        .submit_from::<Compute>("first", keyed())
                        .unwrap()
                        .await
                        .unwrap();
                    assert!(first_result.ptr_eq(&unchanged));
                    runtime
                        .operations
                        .invalidate_from::<Compute>("first", &key)
                        .unwrap();
                    let refreshed = runtime
                        .operations
                        .submit_from::<Compute>("first", keyed())
                        .unwrap()
                        .await
                        .unwrap();
                    assert!(!first_result.ptr_eq(&refreshed));
                    assert_eq!(first.runs.load(Ordering::SeqCst), 2);
                    let preserved = runtime
                        .operations
                        .submit_from::<Compute>("second", keyed())
                        .unwrap()
                        .await
                        .unwrap();
                    assert!(second_result.ptr_eq(&preserved));
                    assert_eq!(second.runs.load(Ordering::SeqCst), 1);
                    runtime
                        .operations
                        .unregister_from("first", Compute::ID)
                        .await
                        .unwrap();
                    assert!(matches!(
                        runtime
                            .operations
                            .submit_from::<Compute>("first", request()),
                        Err(Error::NotRegistered { .. })
                    ));
                    let surviving = runtime
                        .operations
                        .call_as::<Compute>(keyed())
                        .await
                        .unwrap();
                    assert!(second_result.ptr_eq(&surviving));
                    assert_eq!(second.runs.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("lane_refuses_prepare", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let owner = Arc::new(Owner::default());
                    let result = Runtime::<Bank>::builder(Ledger::default(), ())
                        .register(catalog())
                        .operations::<Implemented, _>("app", Lanes::Serial, Arc::clone(&owner))
                        .runtime()
                        .await;
                    let error = result.err().expect("undeclared lane refuses boot");
                    let cause = std::error::Error::source(&error).unwrap();
                    let source = std::error::Error::source(cause).unwrap();
                    assert!(matches!(
                        source.downcast_ref::<Error>(),
                        Some(Error::NotRegistered {
                            domain: "operation lane",
                            ..
                        })
                    ));
                    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
                    assert_eq!(Arc::strong_count(&owner), 1);
                });
        });
    }
    #[cfg(feature = "simulation")]
    {
        scenario_runtime::check("simulation_reboot_rebinds", || {
            let owner = Arc::new(Owner {
                bias: 20,
                ..Owner::default()
            });
            let mut sim = Runtime::<Bank>::builder(Ledger::default(), ())
                .register(catalog())
                .register(Lanes::catalog())
                .operations::<Implemented, _>("app", Lanes::Serial, Arc::clone(&owner))
                .simulate()
                .unwrap();
            assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
            let first = sim.submit_as::<Compute>(request()).unwrap();
            assert_eq!(sim.settle(first).unwrap().value, 27);
            sim.stop().unwrap();
            assert!(matches!(
                sim.submit_as::<Compute>(request()),
                Err(Error::Closing { .. })
            ));
            sim.reboot().unwrap();
            let second = sim.submit_as::<Compute>(request()).unwrap();
            assert_eq!(sim.settle(second).unwrap().value, 27);
            assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);
            assert_eq!(owner.runs.load(Ordering::SeqCst), 2);
            sim.stop().unwrap();
        });
    }
    #[cfg(feature = "simulation")]
    {
        scenario_runtime::check("simulation_replays_against", || {
            let owner = Arc::new(Owner::default());
            let captured = Arc::clone(&owner);
            let initialized = Arc::new(AtomicUsize::new(0));
            let count = Arc::clone(&initialized);
            let mut sim = Runtime::<Bank>::builder(Ledger::default(), ())
                .register(catalog())
                .register(Lanes::catalog())
                .initialize(Duration::from_secs(1), move |handle| {
                    let owner = Arc::clone(&captured);
                    let initialized = Arc::clone(&count);
                    async move {
                        handle.operations.register_operations::<Implemented, _>(
                            "app",
                            Lanes::Open,
                            owner,
                        )?;
                        initialized.fetch_add(1, Ordering::SeqCst);
                        Ok::<(), Error>(())
                    }
                })
                .simulate()
                .unwrap();
            assert_eq!(initialized.load(Ordering::SeqCst), 1);
            let first = sim.submit_as::<Compute>(request()).unwrap();
            assert_eq!(sim.settle(first).unwrap().value, 7);
            sim.stop().unwrap();
            sim.reboot().unwrap();
            assert_eq!(initialized.load(Ordering::SeqCst), 2);
            let second = sim.submit_as::<Compute>(request()).unwrap();
            assert_eq!(sim.settle(second).unwrap().value, 7);
            assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);
            sim.stop().unwrap();
        });
    }
}
