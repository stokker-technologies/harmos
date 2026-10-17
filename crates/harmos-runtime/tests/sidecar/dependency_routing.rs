/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Dependency Authority And Publication Transaction Boundaries.

/* === Modules === */

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
fn candidate(host: &Host, lifecycle: bool, unauthorized: bool, publishing: bool, configuration: bool) -> PluginDefinitions {
    let mut owners = vec![("provider", vec![]), ("caller", vec!["provider"])];
    if unauthorized {
        owners.push(("unauthorized", vec![]));
    }
    let definitions = owners.into_iter().map(|(id, dependencies)| {
        let source = Attachment { id: id.into(), lifecycle, publishing, canonical: true };
        let configuration = if id == "provider" {
            serde_json::to_vec(&host.seed.id).unwrap()
        } else if id == "caller" && (lifecycle || publishing) {
            serde_json::to_vec(&configuration).unwrap()
        } else {
            serde_json::to_vec(&()).unwrap()
        };
        let mut definition = operations::definition(&source, configuration);
        definition.requires = dependencies.into_iter().map(|id| Requirement { id: id.into(), version: "*".parse().unwrap() }).collect();
        definition
    });
    PluginDefinitions::new(definitions, []).unwrap()
}

fn host(runtime: &Runtime<Bank>, provider: Arc<Provider>, lifecycle: bool) -> Host {
    let id = NEXT_PROVIDER_SEED.fetch_add(1, Ordering::SeqCst);
    PROVIDER_SEEDS.lock().unwrap().insert(id, Arc::downgrade(&provider));
    Host {
        operations: runtime.operations.clone(),
        streams: runtime.streams.clone(),
        _provider: provider,
        seed: ProviderSeed { id },
        lifecycle,
        dynamic: false,
        canonical: true,
        capacity: NonZeroUsize::new(4).unwrap(),
        previous_native: None,
        publication_resident: None,
        publication_start: None,
        project_publications: true,
        notices: Arc::new(NoticeLog::default()),
    }
}

async fn published_notices(notices: &NoticeLog, count: usize) {
    loop {
        let changed = notices.changed.notified();
        if notices.values.lock().unwrap().len() >= count {
            return;
        }
        changed.await;
    }
}

#[tokio::test]
async fn initialization_rows_commit() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let mut projected = runtime.streams.watch::<support::Pulse>();
    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
    lifecycle.publication_start = Some(PublicationStart::Commit);
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, true, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    for sample in [41, 42] {
        let next = std::future::poll_fn(|context| futures_core::Stream::poll_next(std::pin::Pin::new(&mut projected), context));
        let row = (next).await.unwrap();
        assert!(matches!(row, harmos_runtime::streams::Item::Row(row) if row.value.sample == sample));
    }
    let source = runtime.streams.publication("meter.pulse", 1).unwrap();
    let rows: Vec<_> = source.query(..).await.unwrap().collect();
    assert_eq!(rows.len(), 2);
    for (row, sample) in rows.iter().zip([41, 42]) {
        assert_eq!(support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample, sample);
    }
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle, projected, source));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn notice_wire_order() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
    lifecycle.publication_start = Some(PublicationStart::Commit);
    lifecycle.project_publications = false;
    lifecycle.dynamic = true;
    let notices = lifecycle.notices.clone();
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, true, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    runtime.operations.call_encoded(PublishPulse::ID, PublishPulse { sample: 43 }.encode_to_vec()).await.unwrap();
    published_notices(&notices, 3).await;
    let retained = notices.values.lock().unwrap().clone();
    assert_eq!(retained.len(), 3);
    let expected_generation = lifecycle.previous_native.as_ref().unwrap().scope().unwrap().generation().get();
    let source = runtime.streams.publication("meter.pulse", 1).unwrap();
    let rows: Vec<_> = source.query(..).await.unwrap().collect();
    assert_eq!(rows.len(), 3);
    for (index, (notice, sample)) in retained.iter().zip([41, 42, 43]).enumerate() {
        let Notice::Published { publication } = notice else { panic!("only committed publications were retained") };
        let cloned = notice.clone();
        let Notice::Published { publication: shared } = &cloned else { unreachable!() };
        assert_eq!(publication.stream(), "meter.pulse");
        assert_eq!(publication.owner(), "caller");
        assert_eq!(publication.schema(), 1);
        assert_eq!(publication.generation(), Some(expected_generation));
        assert_eq!(publication.sequence(), index as u64 + 1);
        assert_eq!(publication.wire_sequence(), index as u64 + 1);
        assert_eq!(publication.missed(), 0);
        assert_eq!(support::Pulse::decode(publication.payload()).unwrap().sample, sample);
        assert_eq!(publication.payload(), rows[index].value.payload);
        assert_eq!(publication.payload().as_ptr(), shared.payload().as_ptr());
        assert_eq!(publication.owner().as_ptr(), shared.owner().as_ptr());
        assert_eq!(publication.published_unix_micros(), shared.published_unix_micros());
        assert!(publication.published_unix_micros().is_some());
        assert_eq!(publication.context(), if index < 2 { Some("initialization") } else { None });
        assert_eq!(publication.context(), shared.context());
    }
    assert!(runtime.streams.query::<support::Pulse>(..).await.unwrap().next().is_none());
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle, source));
    runtime.stop().await.unwrap();
    let Notice::Published { publication } = &retained[0] else { unreachable!() };
    assert_eq!(support::Pulse::decode(publication.payload()).unwrap().sample, 41);
}

#[tokio::test]
async fn shutdown_delivery() {
    let notices = Arc::new(NoticeLog::default());
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
    lifecycle.publication_start = Some(PublicationStart::Commit);
    lifecycle.project_publications = false;
    lifecycle.dynamic = true;
    lifecycle.notices = notices.clone();
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, true, true)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let generation = lifecycle.previous_native.as_ref().unwrap().scope().unwrap().generation().get();
    published_notices(&notices, 2).await;
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
    let retained = notices.values.lock().unwrap();
    assert_eq!(retained.len(), 3);
    for (index, (notice, sample)) in retained.iter().zip([41, 42, 44]).enumerate() {
        let Notice::Published { publication } = notice else { unreachable!() };
        assert_eq!(publication.owner(), "caller");
        assert_eq!(publication.generation(), Some(generation));
        assert_eq!(publication.schema(), 1);
        assert_eq!(publication.sequence(), index as u64 + 1);
        assert_eq!(publication.wire_sequence(), index as u64 + 1);
        assert_eq!(support::Pulse::decode(publication.payload()).unwrap().sample, sample);
        assert_eq!(publication.context(), Some(if sample == 44 { "cleanup" } else { "initialization" }));
    }
}

#[tokio::test]
async fn failed_activation_discards() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
    lifecycle.publication_start = Some(PublicationStart::Refuse);
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, true, false)).unwrap();
    assert!(manager.initialize(&mut lifecycle).await.is_err());
    assert!(runtime.streams.publication("meter.pulse", 1).is_none());
    assert!(runtime.streams.query::<support::Pulse>(..).await.unwrap().next().is_none());
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_activation_discards() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let mut lifecycle = host(&runtime, Arc::new(Provider::default()), true);
    lifecycle.publication_start = Some(PublicationStart::Pause(started.clone()));
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, true, false)).unwrap();
    let mut initialization = Box::pin(manager.initialize(&mut lifecycle));
    (async {
        tokio::select! {
            result = &mut initialization => panic!("initialization ended before cancellation: {result:?}"),
            () = started.notified() => {}
        }
    })
    .await;
    drop(initialization);
    assert!(runtime.streams.publication("meter.pulse", 1).is_none());
    assert!(runtime.streams.query::<support::Pulse>(..).await.unwrap().next().is_none());
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn undeclared_calls_refused() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider::default());
    let mut lifecycle = host(&runtime, provider.clone(), true);
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, false, true)).unwrap();
    let initialized = manager.initialize(&mut lifecycle).await;
    assert_eq!(*provider.observed.lock().unwrap(), [11]);
    assert!(runtime.streams.query::<support::Pulse>(..).await.unwrap().next().is_none());
    assert!(initialized.is_err());
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), [11]);
    assert!(manager.status("caller").unwrap().generation.is_none());
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn dependency_original_manager() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 3, ..Default::default() });
    let mut lifecycle = host(&runtime, provider, false);
    let mut manager = PluginManager::new(candidate(&lifecycle, false, true, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(runtime.operations.call(Forward { value: 7 }).await.unwrap().value, 21);
    let denied = runtime.operations.call(UnauthorizedForward { value: 7 }).await.unwrap_err();
    let harmos_runtime::Error::OperationFailed { source, .. } = denied else { panic!("native source must survive") };
    assert!(
        matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Remote(failure)) if failure.cause().contains("may not invoke"))
    );
    assert_eq!(runtime.operations.call(Forward { value: 8 }).await.unwrap().value, 24);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn dependency_lifecycle_calls() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 1, ..Default::default() });
    let mut lifecycle = host(&runtime, provider.clone(), true);
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), [11]);
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), [11, 22]);
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancellation_settles_handlers() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 1, ..Default::default() });
    let mut lifecycle = host(&runtime, provider.clone(), false);
    let mut manager = PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let invocation = runtime.operations.submit(Forward { value: 0 }).unwrap();
    (provider.started.notified()).await;
    invocation.cancel();
    assert!(matches!(invocation.await, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    (provider.dropped.notified()).await;
    assert_eq!(runtime.operations.call(Forward { value: 9 }).await.unwrap().value, 9);

    let timed = runtime.operations.call(Amplify { value: 0 }).await;
    assert!(matches!(timed, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))));
    (provider.dropped.notified()).await;
    assert_eq!(runtime.operations.call(Amplify { value: 10 }).await.unwrap().value, 10);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn dependency_admission_revocation() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 3, ..Default::default() });
    let mut lifecycle = host(&runtime, provider, false);
    lifecycle.dynamic = true;
    let mut manager = PluginManager::new(candidate(&lifecycle, false, true, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    for (value, expected) in [(7, 21), (8, 24)] {
        let bytes = runtime.operations.call_encoded(Forward::ID, Message::encode_to_vec(&Forward { value })).await.unwrap();
        assert_eq!(Amplified::decode(bytes.as_slice()).unwrap().value, expected);
    }
    let denied = runtime
        .operations
        .call_encoded(UnauthorizedForward::ID, Message::encode_to_vec(&UnauthorizedForward { value: 7 }))
        .await
        .unwrap_err();
    let harmos_runtime::Error::OperationFailed { operation, source } = denied else { panic!("the original native error must survive") };
    assert_eq!(operation, UnauthorizedForward::ID);
    assert!(
        matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Remote(failure)) if failure.cause().contains("may not invoke"))
    );
    assert!(runtime.operations.call_encoded(Amplify::ID, vec![0xff]).await.is_err());
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert!(runtime.operations.submit_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec()).is_err());
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_dependency_capacity() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 1, ..Default::default() });
    let mut lifecycle = host(&runtime, provider.clone(), false);
    lifecycle.dynamic = true;
    lifecycle.capacity = NonZeroUsize::new(1).unwrap();
    let mut manager = PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let invocation = runtime.operations.submit_encoded(Forward::ID, Message::encode_to_vec(&Forward { value: 0 })).unwrap();
    (provider.started.notified()).await;
    let busy = runtime.operations.submit_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec());
    assert!(
        matches!(&busy, Err(harmos_runtime::Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity))),
        "expected native capacity refusal: {:?}",
        busy.as_ref().err()
    );
    invocation.cancel();
    assert!(matches!(invocation.await, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
    (provider.dropped.notified()).await;
    let bytes = runtime.operations.call_encoded(Forward::ID, Message::encode_to_vec(&Forward { value: 9 })).await.unwrap();
    assert_eq!(Amplified::decode(bytes.as_slice()).unwrap().value, 9);
    let timed = runtime.operations.call_encoded(Amplify::ID, Amplify { value: 0 }.encode_to_vec()).await;
    assert!(matches!(timed, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut))));
    (provider.dropped.notified()).await;
    let bytes = runtime.operations.call_encoded(Amplify::ID, Amplify { value: 10 }.encode_to_vec()).await.unwrap();
    assert_eq!(Amplified::decode(bytes.as_slice()).unwrap().value, 10);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn empty_catalog_dependencies() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 1, ..Default::default() });
    let mut lifecycle = host(&runtime, provider.clone(), true);
    lifecycle.dynamic = true;
    let mut manager = PluginManager::new(candidate(&lifecycle, true, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), [11]);
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), [11, 22]);
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn retry_retained_finalization() {
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let provider = Arc::new(Provider { multiplier: 1, ..Default::default() });
    let mut lifecycle = host(&runtime, provider.clone(), false);
    lifecycle.dynamic = true;
    lifecycle.capacity = NonZeroUsize::new(1).unwrap();
    let mut manager = PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let result = runtime.operations.call_encoded(Retry::ID, Message::encode_to_vec(&Retry { value: 1 })).await.unwrap();
    assert_eq!(Amplified::decode(result.as_slice()).unwrap().value, 3);
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(provider.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(provider.finalized.load(Ordering::SeqCst), 1);
    let oversized = runtime.operations.submit_encoded(Retry::ID, Vec::with_capacity(harmos_sidecar::Frame::LIMIT + 1));
    assert!(matches!(oversized, Err(harmos_runtime::Error::OperationFailed { operation, source })
        if operation == Retry::ID && matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Transport(harmos_runtime::services::SidecarError::Refused { code, .. })) if *code == harmos_sidecar::Frame::OVERSIZED)));
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    provider.fail_cleanup.store(true, Ordering::SeqCst);
    let failed = runtime.operations.call_encoded(Retry::ID, Message::encode_to_vec(&Retry { value: 1 })).await;
    assert!(matches!(failed, Err(harmos_runtime::Error::OperationFinalization { operation, .. }) if operation == Retry::ID));
    assert!(runtime.operations.unregister(Retry::ID).await.is_err());
    assert!(matches!(runtime.operations.submit_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec()),
        Err(harmos_runtime::Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity))));
    provider.fail_cleanup.store(false, Ordering::SeqCst);
    runtime.operations.unregister(Retry::ID).await.unwrap();
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 2);
    assert_eq!(provider.attempts.load(Ordering::SeqCst), 4);
    assert_eq!(provider.finalized.load(Ordering::SeqCst), 2);
    runtime.operations.call_encoded(Amplify::ID, Amplify { value: 1 }.encode_to_vec()).await.unwrap();
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn capacity_precedes_encoding() {
    for (dynamic, canonical) in [(true, false), (true, true), (false, true)] {
        let runtime = assembly()
            .operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(256).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        let provider = Arc::new(Provider { multiplier: 1, ..Default::default() });
        let mut lifecycle = host(&runtime, provider.clone(), false);
        lifecycle.dynamic = dynamic;
        lifecycle.canonical = canonical;
        lifecycle.capacity = NonZeroUsize::new(1).unwrap();
        let mut manager = PluginManager::new(candidate(&lifecycle, false, false, false, false)).unwrap();
        let initialized = manager.initialize(&mut lifecycle).await;
        if !canonical {
            assert!(initialized.is_err(), "missing canonical wire contracts must fail activation");
            assert!(provider.observed.lock().unwrap().is_empty());
            assert_eq!(provider.prepared.load(Ordering::SeqCst), 0);
            manager.shutdown(&mut lifecycle).await.unwrap();
            assert!(manager.status("provider").unwrap().generation.is_none());
            drop((manager, lifecycle));
            runtime.stop().await.unwrap();
            continue;
        }
        initialized.unwrap();
        let native = lifecycle.previous_native.as_ref().unwrap();
        let contract = harmos_runtime::operations::WireDeclaration::of::<Forward>().contract().unwrap();
        let target = runtime.operations.capture_callback(native.scope().unwrap(), &contract);
        let target = target.unwrap();
        let invocation = target.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Amplify { value: 0 }.encode_to_vec())).unwrap();
        (provider.started.notified()).await;
        let encoded = AtomicBool::new(false);
        let busy = target.submit_with(NonZeroUsize::new(64).unwrap(), |_| {
            encoded.store(true, Ordering::SeqCst);
            Ok(Amplify { value: 1 }.encode_to_vec())
        });
        assert!(
            matches!(&busy, Err(harmos_runtime::Error::OperationFailed { source, .. }) if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity))),
            "expected native capacity refusal: {:?}",
            busy.as_ref().err()
        );
        assert!(!encoded.load(Ordering::SeqCst));
        if !dynamic {
            let busy = runtime.operations.submit(Forward { value: 1 }).unwrap().await;
            assert!(matches!(busy, Err(harmos_runtime::Error::OperationFailed { source, .. })
                if matches!(source.downcast_ref::<NativeError>(), Some(NativeError::Capacity))));
        }
        invocation.cancel();
        assert!(matches!(invocation.await, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::Cancelled))));
        let bytes =
            target.submit_with(NonZeroUsize::new(64).unwrap(), |_| Ok(Amplify { value: 9 }.encode_to_vec())).unwrap().await.unwrap();
        assert_eq!(Amplified::decode(&bytes).unwrap().value, 9);
        if !dynamic {
            let response = runtime.operations.call(Forward { value: 11 }).await.unwrap();
            assert_eq!(response.value, 11);
        }
        manager.shutdown(&mut lifecycle).await.unwrap();
        drop((manager, lifecycle));
        runtime.stop().await.unwrap();
    }
}

/* === Tests === */
