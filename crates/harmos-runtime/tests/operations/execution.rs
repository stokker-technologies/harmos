/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use futures_util::FutureExt;

use super::*;

#[cfg(feature = "simulation")]
struct InitializingBank;

#[cfg(feature = "simulation")]
impl harmos_runtime::Application for InitializingBank {
    type Lanes = Lanes;
    type State = <Bank as harmos_runtime::Application>::State;
    type Transaction = <Bank as harmos_runtime::Application>::Transaction;
    type Record = <Bank as harmos_runtime::Application>::Record;
    type Streams = <Bank as harmos_runtime::Application>::Streams;
}

#[cfg(feature = "simulation")]
impl harmos_runtime::Lifecycle for InitializingBank {
    type Config = ();
    type Error = Error;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Error> {
        Ok(Self)
    }

    async fn initialize(&mut self, context: &Self::Context) -> Result<(), Error> {
        context.runtime.register::<Operations>(Lanes::Open).await?;
        context.resource::<AtomicUsize>().expect("the test registers its initialization counter").fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test(start_paused = true)]
async fn retry_retains_invocation_identity() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    let mut invocation = runtime.operation(Request { failures: 2, ..request() }).start().await.unwrap();
    let id = invocation.id();
    let generation = invocation.generation();
    let response = (&mut invocation).await.unwrap();
    assert_eq!(invocation.id(), id);
    assert_eq!(invocation.generation(), generation);
    assert_eq!(invocation.status().attempt(), 2);
    assert_eq!(invocation.status().phase(), harmos_runtime::operations::OperationPhase::Settled);
    assert_eq!(response.attempts, 3);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(*owner.seen.lock().unwrap(), vec![id; 3]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn errors_never_retry() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    for failure in [FailureKind::Permanent, FailureKind::Unauthorized, FailureKind::Invalid, FailureKind::Cancelled] {
        assert!(matches!(
            runtime.operation(Request { failures: 10, failure, ..request() }).run().await,
            Err(Error::OperationFailed { .. })
        ));
    }
    assert_eq!(owner.runs.load(Ordering::SeqCst), 4);
    runtime.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn ambiguous_timeout_replay() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Slow>>(Lanes::Open).await.unwrap();
    assert!(matches!(runtime.operation(Slow(request())).run().await, Err(Error::OperationRefused(Refusal::TimedOut))));
    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cache_serves_hits() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    let keyed = || Request { keyed: true, ..request() };

    let first = runtime.operation(keyed()).run().await.unwrap();
    let second = runtime.operation(keyed()).run().await.unwrap();

    // A hit reuses the cached response without constructing the operation.
    assert!(first.ptr_eq(&second));
    assert_eq!(owner.runs.load(Ordering::SeqCst), 1);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn uncacheable_calls_rerun() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();

    // Invalid Requests Bypass The Cache
    let invalid = || Request { keyed: true, valid: false, ..request() };
    for _ in 0..2 {
        assert!(matches!(runtime.operation(invalid()).run().await, Err(Error::OperationFailed { .. })));
    }
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);

    // Unkeyed Requests Always Run
    for _ in 0..2 {
        runtime.operation(request()).run().await.unwrap();
    }
    assert_eq!(owner.runs.load(Ordering::SeqCst), 2);

    // Capacity Zero Disables Caching
    runtime.operations.set_cache_capacity(0);
    let keyed = || Request { keyed: true, ..request() };
    for _ in 0..2 {
        runtime.operation(keyed()).run().await.unwrap();
    }
    assert_eq!(owner.runs.load(Ordering::SeqCst), 4);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn unregister_reserves_owner() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    let opened = Arc::new(Notify::new());
    let gate = Arc::new(Notify::new());
    let invocation = runtime
        .operation(Request {
            keyed: true,
            gate: Some((Arc::clone(&opened), Arc::clone(&gate))),
            ..request()
        })
        .start()
        .await
        .unwrap();
    opened.notified().await;
    // One poll reserves the id; dropping the waiter interrupts the unregister.
    assert!(runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Compute::ID).unwrap()).now_or_never().is_none());
    assert!(runtime.operations.definition(Compute::ID).is_err());
    assert!(matches!(runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await, Err(Error::AlreadyRegistered { .. })));
    gate.notify_one();
    assert!(matches!(invocation.await, Err(Error::OperationRefused(Refusal::Cancelled))));
    // Settled work does not release an interrupted explicit unregister's reservation.
    assert!(matches!(runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await, Err(Error::AlreadyRegistered { .. })));
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Compute::ID).unwrap()).await.unwrap();
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    assert_eq!(runtime.operation(Request { keyed: true, ..request() }).run().await.unwrap().value, 7);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 2);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn operations_use_structured() {
    let runtime = runtime().await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    runtime.operations.register::<super::Single<Parent>>(Lanes::Serial).await.unwrap();
    assert_eq!(runtime.operation(Parent(request())).run().await.unwrap().value, 7);
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Compute::ID).unwrap()).await.unwrap();
    runtime.operations.register::<super::Single<Compute>>(Lanes::Serial).await.unwrap();
    assert!(
        matches!(runtime.operation(Parent(request())).run().await, Err(Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<Failed>(), Some(Failed::Invocation(harmos_runtime::operations::InvocationError::Failed { source, .. })) if matches!(source.downcast_ref::<Error>(), Some(Error::OperationLaneCycle { .. }))))
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_ancestor_reentry() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Concurrent).await.unwrap();
    runtime.operations.register::<super::Single<Parent>>(Lanes::Concurrent).await.unwrap();

    assert!(
        matches!(runtime.operation(Parent(request())).run().await, Err(Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<Failed>(), Some(Failed::Invocation(harmos_runtime::operations::InvocationError::Failed { source, .. })) if matches!(source.downcast_ref::<Error>(), Some(Error::OperationLaneCycle { .. }))))
    );
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);

    // An independent root still uses this bounded lane normally.
    assert_eq!(runtime.operation(request()).run().await.unwrap().value, 7);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_releases_bindings() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    let weak = Arc::downgrade(&owner);
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    runtime.operation(Request { keyed: true, ..request() }).run().await.unwrap();
    runtime.shutdown().await.unwrap();
    assert!(matches!(runtime.operation(request()).start().await, Err(Error::Closing { .. })));
    drop(owner);
    drop(runtime);
    assert!(weak.upgrade().is_none());
}

#[tokio::test]
async fn handler_panic_settles() {
    let runtime = runtime().await;
    runtime.operations.register::<super::Single<Compute>>(Lanes::Open).await.unwrap();
    assert!(matches!(runtime.operation(Request { value: usize::MAX, ..request() }).run().await, Err(Error::OperationPanicked { .. })));
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Compute::ID).unwrap()).await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn operation_catalog_binding() {
    let owner = Arc::new(Owner::default());
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    let definitions = harmos_runtime::operations::Declarations::of::<Operations>().unwrap();
    assert_eq!(definitions[0].id.as_str(), Compute::ID);
    assert_eq!(definitions[1].id.as_str(), Slow::ID);
    runtime.operations.register::<Operations>(Lanes::Open).await.unwrap();
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.operation(request()).run().await.unwrap().value, 7);

    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Compute::ID).unwrap()).await.unwrap();
    assert!(matches!(runtime.operations.register::<Operations>(Lanes::Open).await, Err(Error::AlreadyRegistered { .. })));
    assert!(runtime.operations.definition(Compute::ID).is_err());
    assert!(runtime.operations.definition(Slow::ID).is_ok());
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn catalog_boot_atomic() {
    let owner = Arc::new(Owner::default());
    let result = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.resource::<Owner>(owner.clone());
        setup.operations::<Operations>(Lanes::Open).operations::<Operations>(Lanes::Open);
    })
    .await;
    assert!(matches!(result, Err(harmos_runtime::StartupError::DuplicateCatalog { .. })));
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
    assert_eq!(Arc::strong_count(&owner), 1);

    let owner = Arc::new(Owner { bias: 20, ..Owner::default() });
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        setup.settings().register(catalog());
        setup.resource::<Owner>(owner.clone());
        setup.operations::<Operations>(Lanes::Serial);
    })
    .await
    .unwrap();
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.operation(request()).run().await.unwrap().value, 27);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn providers_route_explicitly() {
    let owner = Arc::new(Owner { bias: 10, ..Owner::default() });
    let runtime = runtime_with(|setup| {
        setup.resource::<Owner>(owner.clone());
    })
    .await;
    for id in ["first", "second"] {
        runtime
            .plugin
            .register_embedded::<Operations, Lanes>(
                harmos_runtime::plugin::PluginDefinition {
                    id: id.into(),
                    version: "1.0.0".parse().unwrap(),
                    artifact: harmos_runtime::plugin::PluginArtifact::Embedded { revision: "fixture".into() },
                    requires: vec![],
                    configuration: b"null".to_vec(),
                    contributions: Default::default(),
                },
                Default::default(),
                Lanes::Open,
            )
            .await
            .unwrap();
    }
    runtime.plugin.initialize().await.unwrap();
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    assert!(matches!(runtime.operation(request()).start().await, Err(Error::InvalidInput { field: "operation provider", .. })));
    assert!(matches!(
        runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("host", Compute::ID).unwrap()).await,
        Err(Error::NotRegistered { .. })
    ));
    assert_eq!(runtime.operations.definition_from("first", Compute::ID).unwrap().id.as_str(), Compute::ID);
    let keyed = || Request { keyed: true, ..request() };
    let first_result = runtime.operations.submit_from::<Compute>("first", keyed()).unwrap().await.unwrap();
    let second_result = runtime.operations.submit_from::<Compute>("second", keyed()).unwrap().await.unwrap();
    assert_eq!(first_result.value, 17);
    assert_eq!(second_result.value, 17);
    assert!(!first_result.ptr_eq(&second_result));
    let again = runtime.operations.submit_from::<Compute>("second", keyed()).unwrap().await.unwrap();
    assert!(second_result.ptr_eq(&again));
    assert_eq!(owner.runs.load(Ordering::SeqCst), 2);
    let key = Compute::key(&keyed()).unwrap();
    assert!(matches!(runtime.operations.invalidate::<Compute>(&key), Err(Error::InvalidInput { field: "operation provider", .. })));
    assert!(matches!(runtime.operations.invalidate_from::<Compute>("missing", &key), Err(Error::NotRegistered { .. })));
    let unchanged = runtime.operations.submit_from::<Compute>("first", keyed()).unwrap().await.unwrap();
    assert!(first_result.ptr_eq(&unchanged));
    runtime.operations.invalidate_from::<Compute>("first", &key).unwrap();
    let refreshed = runtime.operations.submit_from::<Compute>("first", keyed()).unwrap().await.unwrap();
    assert!(!first_result.ptr_eq(&refreshed));
    assert_eq!(owner.runs.load(Ordering::SeqCst), 3);
    let preserved = runtime.operations.submit_from::<Compute>("second", keyed()).unwrap().await.unwrap();
    assert!(second_result.ptr_eq(&preserved));
    assert_eq!(owner.runs.load(Ordering::SeqCst), 3);
    runtime.operations.unregister(&harmos_runtime::QualifiedId::qualify("first", Compute::ID).unwrap()).await.unwrap();
    assert!(matches!(runtime.operations.submit_from::<Compute>("first", request()), Err(Error::NotRegistered { .. })));
    let surviving = runtime.operation(keyed()).run().await.unwrap();
    assert!(second_result.ptr_eq(&surviving));
    assert_eq!(owner.runs.load(Ordering::SeqCst), 3);
    runtime.shutdown().await.unwrap();
}

#[cfg(feature = "simulation")]
#[test]
fn simulation_reboot_rebinds() {
    let owner = Arc::new(Owner { bias: 20, ..Owner::default() });
    let captured = Arc::clone(&owner);
    let mut sim = Runtime::<Bank>::simulate_with(Ledger::default(), (), 7, move |setup| {
        setup.settings().register(catalog());
        setup.resource::<Owner>(captured.clone());
        setup.operations::<Operations>(Lanes::Serial);
    })
    .unwrap();
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 0);
    let first = sim.submit::<Compute>(request()).unwrap();
    assert_eq!(sim.settle(first).unwrap().value, 27);
    sim.stop().unwrap();
    assert!(matches!(sim.submit::<Compute>(request()), Err(Error::Closing { .. })));
    sim.reboot().unwrap();
    let second = sim.submit::<Compute>(request()).unwrap();
    assert_eq!(sim.settle(second).unwrap().value, 27);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);
    assert_eq!(owner.runs.load(Ordering::SeqCst), 2);
    sim.stop().unwrap();
}

#[cfg(feature = "simulation")]
#[test]
fn reboot_reruns_initialize() {
    let owner = Arc::new(Owner::default());
    let captured = Arc::clone(&owner);
    let initialized = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&initialized);
    let mut sim = Runtime::<InitializingBank>::simulate_with(Ledger::default(), (), 7, move |setup| {
        setup.resource::<Owner>(captured.clone());
        setup.resource::<AtomicUsize>(count.clone());
    })
    .unwrap();
    assert_eq!(initialized.load(Ordering::SeqCst), 1);
    let first = sim.submit::<Compute>(request()).unwrap();
    assert_eq!(sim.settle(first).unwrap().value, 7);
    sim.stop().unwrap();
    sim.reboot().unwrap();
    assert_eq!(initialized.load(Ordering::SeqCst), 2);
    let second = sim.submit::<Compute>(request()).unwrap();
    assert_eq!(sim.settle(second).unwrap().value, 7);
    assert_eq!(owner.prepared.load(Ordering::SeqCst), 2);
    sim.stop().unwrap();
}
