/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Publication Windows, Capture And Observer Activation.

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

#[tokio::test]
async fn capture_survives_reload() {
    let attachments = Arc::new(AtomicUsize::new(0));
    let source = meter(attachments.clone());
    let definitions = operations::inspected(&source, serde_json::to_vec(&()).unwrap());
    let calls = Supervised::new(source, (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice));
    let rows = Arc::new(Mutex::new(Vec::new()));
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .attach_streams(Recorder { cursor: Cursor::ORIGIN, rows: rows.clone() })
        .runtime()
        .await
        .unwrap();
    let mut fixture = Managed::from_runtime(
        runtime,
        definitions,
        calls,
        Lanes::NativeOpen,
        std::num::NonZeroUsize::new(16).unwrap(),
        harmos_runtime::services::POLICY,
    );
    fixture.publication().unwrap();
    fixture
        .runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    fixture.call(PublishPulse { sample: 41 }).await.unwrap();
    let stream = fixture.runtime.streams.publication("meter.pulse", 1).unwrap();
    assert_eq!(stream.definition().owner(), Some("meter"));
    assert!(!stream.definition().schema().unwrap().root().unwrap().fields().is_empty());

    fixture.shutdown().await.unwrap();
    assert_eq!(stream.query(..).await.unwrap().count(), 1);
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    fixture.call(PublishPulse { sample: 42 }).await.unwrap();
    let samples: Vec<_> = stream
        .query(..)
        .await
        .unwrap()
        .map(|row| {
            assert!(row.value.published_unix_micros.is_some(), "sidecar submission time must reach the retained stream");
            support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample
        })
        .collect();
    assert_eq!(samples, [41, 42]);
    assert_eq!(attachments.load(Ordering::SeqCst), 2);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();

    let recorded = rows.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert!(recorded.iter().all(|(id, _)| id == "meter.pulse"));
    assert_eq!(recorded.iter().map(|(_, bytes)| support::Pulse::decode(bytes.as_slice()).unwrap().sample).collect::<Vec<_>>(), [41, 42]);
}

#[tokio::test]
async fn demand_survives_reload() {
    let gate = Gate::default();
    let seed = GateSeed::new(&gate);
    let capacity = std::num::NonZeroUsize::new(4).unwrap();
    let source = harmos_sidecar::PluginNative::<Ticking>::new(capacity).unwrap();
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        capacity,
        vec![TICKING_DEMAND.name()],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    let demands = fixture.demands();
    let held = demands.hold(&TICKING_DEMAND.name()).unwrap();
    let streams = fixture.runtime.streams.clone();
    {
        let initializing = fixture.initialize();
        tokio::pin!(initializing);
        tokio::select! {
            result = &mut initializing => panic!("initialization completed before gate release: {result:?}"),
            _ = gate.started.notified() => {}
        }
        assert!(streams.publication("ticking.pulse", 1).is_none(), "blocked initialization cannot expose staged streams");
        gate.release.notify_one();
        initializing.await.unwrap();
    }
    ready(fixture.ready().clone()).await;
    let stream = streams.publication("ticking.pulse", 1).unwrap();
    let mut samples = stream.watch();
    row(&mut samples).await;
    let original = observers(&fixture.runtime);
    assert_eq!(original.len(), 1);
    let retiring = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    fixture.pause_retirement(retiring.clone(), release.clone());
    {
        let shutdown = fixture.shutdown();
        tokio::pin!(shutdown);
        tokio::select! {
            result = &mut shutdown => panic!("shutdown completed before retirement release: {result:?}"),
            _ = retiring.notified() => {}
        }
        // The Original Observer Still Owns Its Control After Ordinary Admission Closes.
        let mut during_retirement = stream.watch();
        tokio::select! {
            result = &mut shutdown => panic!("shutdown completed before retirement release: {result:?}"),
            _ = row(&mut during_retirement) => {}
        }
        release.notify_one();
        shutdown.await.unwrap();
    }
    drop(samples);
    assert!(observer_definitions(&fixture.runtime).is_empty());

    let replacement_seed = GateSeed::new(&gate);
    {
        let reloading = fixture.reload(serde_json::to_vec(&replacement_seed.id).unwrap());
        tokio::pin!(reloading);
        tokio::select! {
            result = &mut reloading => panic!("replacement initialized before gate release: {result:?}"),
            _ = gate.started.notified() => {}
        }
        gate.release.notify_one();
        reloading.await.unwrap();
    }
    ready(fixture.ready().clone()).await;
    let replacement = observers(&fixture.runtime);
    assert_eq!(replacement.len(), 1);
    assert!(replacement.keys().all(|key| !original.contains_key(key)), "replacement keeps demand but gets a fresh generation");
    let mut after_restart = streams.publication("ticking.pulse", 1).unwrap().watch();
    row(&mut after_restart).await;
    drop(held);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn batches_keep_order() {
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Batches>::new(capacity).unwrap();
    // A Decoded 100-Row Batch Outgrows The Usual 4 KiB Preparation Budget
    let runtime = assembly()
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: std::num::NonZeroUsize::new(64 * 1024),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    let definitions = super::super::operations::inspected(&source, serde_json::to_vec(&()).unwrap());
    let resident = harmos_runtime::sidecar::Supervised::new(
        source,
        (|_: &harmos_runtime::services::Service<crate::sidecar::support::Bank>, _: harmos_runtime::sidecar::Notice| {})
            as fn(&harmos_runtime::services::Service<crate::sidecar::support::Bank>, _),
    );
    let mut fixture = Managed::from_runtime(runtime, definitions, resident, Lanes::NativeOpen, capacity, harmos_runtime::services::POLICY);
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    fixture.shutdown().await.unwrap();

    let rows = |id: &str| {
        let stream = fixture.runtime.streams.publication(id, 1).unwrap();
        async move {
            stream
                .query(..)
                .await
                .unwrap()
                .map(|row| (support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample, row.value.context.clone()))
                .collect::<Vec<_>>()
        }
    };
    let a = || Some("execution-a".to_owned());
    let b = || Some("execution-b".to_owned());
    assert_eq!(rows("batches.rows").await, vec![(1, a()), (2, a()), (3, a()), (4, None), (5, b()), (6, b())]);
    assert_eq!(rows("batches.raw").await, vec![(1, None), (2, None), (3, None)]);
    let stamps = |id: &str| {
        let stream = fixture.runtime.streams.publication(id, 1).unwrap();
        async move {
            stream
                .query(..)
                .await
                .unwrap()
                .map(|row| (support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample, row.value.published_unix_micros.unwrap()))
                .collect::<Vec<_>>()
        }
    };
    // 100 Distinct Stamps Exactly One Millisecond Apart
    assert_eq!(stamps("batches.timed").await, (0..100).map(|sample| (sample, TIMED_BASE + 1_000 * sample as i64)).collect::<Vec<_>>());
    // Kit Clock Monotonicity Is Exercised In The Publisher's Controlled-Clock Unit Story.
    assert_eq!(stamps("batches.clocked").await, (0..100).map(|sample| (sample, TIMED_BASE + 1_000 * sample as i64)).collect::<Vec<_>>());
    // The Backwards Row And Everything After It Stay Unpublished
    assert_eq!(stamps("batches.stamped").await, vec![(1, TIMED_BASE), (2, TIMED_BASE + 1_000), (3, TIMED_BASE + 1_000), (4, TIMED_BASE),]);
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn held_ingest_lossless() {
    let witness = super::super::witness::Witness::open();
    let capacity = std::num::NonZeroUsize::new(4).unwrap();
    let source = harmos_sidecar::PluginNative::<Flood>::new(capacity).unwrap();
    let mut fixture =
        Managed::attach(source, serde_json::to_vec(witness.key()).unwrap(), Lanes::NativeOpen, capacity, vec![], std::convert::identity)
            .await;
    // The Host's Ingest Follows Before The Producer Starts
    let mut feed = fixture.runtime.streams.follow(Cursor::ORIGIN);
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;

    // Hold The First Ingest Batch: A Wider Publication Cannot Complete Until Ack.
    witness.marked("publication.started").await;
    let mut samples = Vec::new();
    let mut batches = 0;
    (async {
        while samples.len() < FLOOD_ROWS as usize {
            let batch = feed.next().await.expect("the feed stays live");
            assert!(batch.incomplete().is_empty(), "the window dropped an unread row");
            samples.extend(
                batch
                    .publications()
                    .filter(|(definition, ..)| definition.id() == "flood.rows")
                    .map(|(_, _, row)| support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample),
            );
            batches += 1;
            if batches == 1 {
                assert_eq!(witness.count("publication.completed"), 0, "the unacknowledged first window must pace its wider publication");
            }
        }
    })
    .await;

    // Every Row Arrived Once, In Order, Through A Window Of 8
    assert_eq!(samples, (0..FLOOD_ROWS).collect::<Vec<_>>());
    assert!(batches > 1, "the producer was paced across ingest batches");
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn named_streams_isolated() {
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Twins>::new(capacity).unwrap();
    let mut fixture =
        Managed::attach(source, serde_json::to_vec(&()).unwrap(), Lanes::NativeOpen, capacity, vec![], std::convert::identity).await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    fixture.shutdown().await.unwrap();

    for (id, expected) in [("left", vec![10, 11]), ("right", vec![20])] {
        let stream = fixture.runtime.streams.publication(&format!("twins.{id}"), 1).unwrap();
        let samples: Vec<_> =
            stream.query(..).await.unwrap().map(|row| support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample).collect();
        assert_eq!(samples, expected);
        let contexts: Vec<_> = stream.query(..).await.unwrap().map(|row| row.value.context.clone()).collect();
        assert_eq!(contexts, if id == "left" { vec![Some("execution-a".into()), Some("execution-b".into())] } else { vec![None] });
        assert_eq!(stream.definition().id(), format!("twins.{id}"));
        assert_eq!(
            stream.definition().schema().unwrap(),
            &harmos_runtime::schema::graph::Schema::from_declaration(
                <support::Pulse as harmos_runtime::schema::Described>::DEFINITION,
                &harmos_runtime::config::SchemaSettings::default()
            )
            .unwrap()
        );
    }
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn projection_preserves_evidence() {
    let capacity = std::num::NonZeroUsize::new(2).unwrap();
    let source = harmos_sidecar::PluginNative::<Twins>::new(capacity).unwrap();
    let mut fixture = Managed::attach(source, serde_json::to_vec(&()).unwrap(), Lanes::NativeOpen, capacity, vec![], |resident| {
        resident.ingest::<Left>().ingest::<Right>()
    })
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    fixture.shutdown().await.unwrap();

    let projected: Vec<_> = fixture.runtime.streams.query::<support::Pulse>(..).await.unwrap().map(|row| row.value.sample).collect();
    assert_eq!(projected, [10, 20, 11]);
    for (id, expected) in [("twins.left", vec![10, 11]), ("twins.right", vec![20])] {
        let source = fixture.runtime.streams.publication(id, 1).unwrap();
        assert_eq!(source.definition().owner(), Some("twins"));
        let retained: Vec<_> =
            source.query(..).await.unwrap().map(|row| support::Pulse::decode(row.value.payload.as_slice()).unwrap().sample).collect();
        assert_eq!(retained, expected);
    }
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn shared_row_independence() {
    use std::sync::atomic::Ordering::SeqCst;
    let counts = ObserverCounts::default();
    let seed = ObserverSeed::new(&counts);
    let source = harmos_sidecar::PluginNative::<SampledTwins>::new(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        std::num::NonZeroUsize::new(4).unwrap(),
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    let runtime = &fixture.runtime;
    let left = runtime.streams.publication("twins.left", 1).unwrap();
    let right = runtime.streams.publication("twins.right", 1).unwrap();
    assert_eq!(left.definition().schema(), right.definition().schema());
    assert_eq!(counts.started[0].load(SeqCst), 0);
    assert_eq!(counts.started[1].load(SeqCst), 0);

    let mut left_rows = left.watch();
    row(&mut left_rows).await;
    assert_eq!(counts.started[0].load(SeqCst), 1);
    assert_eq!(counts.started[1].load(SeqCst), 0);
    assert_eq!(right.query(..).await.unwrap().count(), 0);

    let mut right_rows = right.watch();
    row(&mut right_rows).await;
    assert_eq!(counts.started[1].load(SeqCst), 1);
    drop(left_rows);
    counts.until_stopped(0).await;
    assert_eq!(counts.stopped[1].load(SeqCst), 0);
    assert_eq!(counts.released.load(SeqCst), 0);
    assert_eq!(support::Pulse::decode(left.query(..).await.unwrap().next().unwrap().value.payload.as_slice()).unwrap().sample, 101);
    assert_eq!(support::Pulse::decode(right.query(..).await.unwrap().next().unwrap().value.payload.as_slice()).unwrap().sample, 201);

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert_eq!(counts.stopped[0].load(SeqCst), 1);
    assert_eq!(counts.stopped[1].load(SeqCst), 1);
    assert_eq!(counts.released.load(SeqCst), 1);
}

#[tokio::test]
async fn catalog_precedes_initialization() {
    use harmos_runtime::plugin::{PluginArtifact, PluginContributions, PluginDefinition, PluginDefinitions};
    use std::sync::atomic::Ordering::SeqCst;

    for case in 0..5 {
        let counts = ObserverCounts::default();
        let seed = ObserverSeed::new(&counts);
        let capacity = std::num::NonZeroUsize::new(4).unwrap();
        let inner = harmos_sidecar::PluginNative::<SampledTwins>::new(capacity).unwrap();
        let mut observers = inner.observers();
        let reason = match case {
            0 => {
                observers[0].interval = Duration::ZERO;
                "interval must be positive"
            }
            1 => {
                observers.pop();
                "observers differ from the admitted catalog"
            }
            2 => {
                observers[0].interval += Duration::from_nanos(1);
                "observers differ from the admitted catalog"
            }
            3 => {
                observers[0].streams.push("ungranted".into());
                "artifact publication grant"
            }
            _ => {
                observers.push(observers[0].clone());
                "identities must be nonempty, unique"
            }
        };
        let inert = harmos_sidecar::inventory::observers::validate(&observers, &inner.publications());
        let definition = PluginDefinition {
            id: inner.owner().into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::LocalSidecar { revision: "fixture".into() },
            configuration: serde_json::to_vec(&seed.id).unwrap(),
            requires: vec![],
            contributions: PluginContributions {
                operations: inner.operations().unwrap(),
                wire_contracts: inner.operation_contracts().unwrap().unwrap(),
                required_contracts: inner.required_contracts().unwrap(),
                lifecycle: inner.sdk_lifecycle().unwrap(),
                streams: inner.publications(),
                observers: observers.clone(),
                listeners: inner.listener_declarations().unwrap().unwrap(),
                ..PluginContributions::default()
            },
        };
        let definitions = PluginDefinitions::new([definition], []);
        if matches!(case, 0 | 3 | 4) {
            assert!(inert.unwrap_err().to_string().contains(reason), "case {case}");
            assert!(definitions.is_err(), "invalid inventory fails manager preflight before attachment");
            assert_eq!(counts.initialized.load(SeqCst), 0);
            assert!(OBSERVER_SEEDS.lock().unwrap().contains_key(&seed.id), "preflight did not prepare resources");
            assert_eq!(counts.released.load(SeqCst), 0);
            continue;
        }
        inert.unwrap();
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peer = Advertised { inner, observers, attached: attempts.clone() };
        let mut fixture =
            Managed::from_definition(peer, definitions.unwrap(), Lanes::NativeOpen, capacity, vec![], std::convert::identity).await;
        fixture.publication().unwrap();
        assert!(fixture.initialize().await.is_err());
        let status = service_status(fixture.runtime.operations.watch_residents(), "twin-observers", |status| {
            matches!(status, ServiceStatus::Inactive { .. })
        })
        .await;
        assert!(
            matches!(status, ServiceStatus::Inactive { cause: ServiceExit::Application(ref message), .. } if message.contains(reason)),
            "case {case}: {status:?}"
        );
        assert_eq!(attempts.load(SeqCst), 1, "application admission refusal is terminal under the original service policy");
        assert_eq!(counts.initialized.load(SeqCst), 0, "case {case}");
        assert!(OBSERVER_SEEDS.lock().unwrap().contains_key(&seed.id), "wire mismatch precedes preparation");
        assert!(!*fixture.ready().borrow());
        assert_eq!(counts.released.load(SeqCst), 0);
        assert!(matches!(fixture.runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
    }
}

#[tokio::test]
async fn catalog_order_admission() {
    use harmos_sidecar::Attach;
    use std::sync::atomic::Ordering::SeqCst;

    let counts = ObserverCounts::default();
    let seed = ObserverSeed::new(&counts);
    let inner = harmos_sidecar::PluginNative::<SampledTwins>::new(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let mut observers = inner.observers();
    observers.reverse();
    for observer in &mut observers {
        observer.streams.reverse();
        observer.demand_keys.reverse();
    }
    let peer = Advertised { inner, observers, attached: Arc::new(std::sync::atomic::AtomicUsize::new(0)) };
    let mut fixture = Managed::attach(
        peer,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        std::num::NonZeroUsize::new(4).unwrap(),
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    assert_eq!(counts.initialized.load(SeqCst), 1);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn shutdown_signals_siblings() {
    use std::sync::atomic::Ordering::SeqCst;
    let counts = ObserverCounts::default();
    counts.coordinated.store(true, SeqCst);
    let seed = ObserverSeed::new(&counts);
    let source = harmos_sidecar::PluginNative::<SampledTwins>::new(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        std::num::NonZeroUsize::new(4).unwrap(),
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    let runtime = &fixture.runtime;
    assert_eq!(observer_definitions(runtime).len(), 2);
    let mut left = runtime.streams.publication("twins.left", 1).unwrap().watch();
    let mut right = runtime.streams.publication("twins.right", 1).unwrap().watch();
    row(&mut left).await;
    row(&mut right).await;
    let mut statuses = runtime.operations.watch_residents();
    (statuses.wait_for(|statuses| {
        let observers: Vec<_> =
            statuses.values().filter(|status| status.kind == harmos_runtime::operations::resident::Kind::Observer).collect();
        observers.len() == 2
            && observers
                .iter()
                .all(|status| status.lifecycle.phase() == harmos_runtime::operations::OperationPhase::Running && status.leases == 1)
    }))
    .await
    .unwrap();
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert_eq!(counts.stopped[0].load(SeqCst), 1);
    assert_eq!(counts.stopped[1].load(SeqCst), 1);
    assert_eq!(counts.released.load(SeqCst), 1);
    assert!(observer_definitions(&fixture.runtime).is_empty());
}

#[tokio::test]
async fn stop_preserves_failure() {
    use std::sync::atomic::Ordering::SeqCst;
    let counts = ObserverCounts::default();
    counts.stop_panics.store(true, SeqCst);
    let seed = ObserverSeed::new(&counts);
    let source = harmos_sidecar::PluginNative::<SampledTwins>::new(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let mut fixture = Managed::attach(
        source,
        serde_json::to_vec(&seed.id).unwrap(),
        Lanes::NativeOpen,
        std::num::NonZeroUsize::new(4).unwrap(),
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    ready(fixture.ready().clone()).await;
    let runtime = &fixture.runtime;
    let mut left = runtime.streams.publication("twins.left", 1).unwrap().watch();
    row(&mut left).await;
    let error = fixture.runtime.operations.stop_observers().await.expect_err("native cleanup failure reaches the manager");
    assert_eq!(counts.stop_attempts.load(SeqCst), 1);
    assert_eq!(counts.stopped[0].load(SeqCst), 0);
    assert_eq!(counts.released.load(SeqCst), 0, "provider survives failed observer cleanup");
    let mut cause: &(dyn std::error::Error + 'static) = &error;
    loop {
        if cause.to_string().contains("the observer panicked while stopping") {
            break;
        }
        if let Some(harmos_runtime::Error::TaskFailed { source, .. }) = cause.downcast_ref::<harmos_runtime::Error>() {
            cause = source.as_ref();
        } else if let Some(harmos_runtime::Error::PluginLifecycle { source }) = cause.downcast_ref::<harmos_runtime::Error>() {
            cause = source.as_ref();
        } else if let Some(harmos_runtime::plugin::PluginFailure::Cleanup(failures)) =
            cause.downcast_ref::<harmos_runtime::plugin::PluginFailure>()
        {
            assert_eq!(failures.len(), 1);
            cause = &failures[0].error;
        } else {
            cause = cause.source().expect("structured native exit is retained");
        }
    }
    counts.stop_panics.store(false, SeqCst);
    let _ = fixture.runtime.operations.stop_observers().await;
    assert_eq!(counts.stop_attempts.load(SeqCst), 2);
    assert_eq!(counts.started[0].load(SeqCst), 1);
    assert_eq!(counts.stopped[0].load(SeqCst), 1);
    let _ = fixture.shutdown().await;
    let _ = fixture.runtime.stop().await;
    assert_eq!(counts.released.load(SeqCst), 1);
}

#[tokio::test]
async fn dropped_manager_recovery() {
    use std::sync::atomic::Ordering::SeqCst;
    (async {
        let counts = ObserverCounts::default();
        counts.stop_panics.store(true, SeqCst);
        let seed = ObserverSeed::new(&counts);
        let source = harmos_sidecar::PluginNative::<SampledTwins>::new(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
        let mut fixture = Managed::attach(
            source,
            serde_json::to_vec(&seed.id).unwrap(),
            Lanes::NativeOpen,
            std::num::NonZeroUsize::new(4).unwrap(),
            vec![],
            std::convert::identity,
        )
        .await;
        fixture.publication().unwrap();
        fixture.initialize().await.unwrap();
        ready(fixture.ready().clone()).await;
        let mut left = fixture.runtime.streams.publication("twins.left", 1).unwrap().watch();
        row(&mut left).await;
        let (runtime, cleanup) = fixture.drop_manager();
        cleanup.join().await.expect_err("failed original observer cleanup is diagnosable after manager destruction");
        assert_eq!(counts.started[0].load(SeqCst), 1);
        assert_eq!(counts.stop_attempts.load(SeqCst), 1);
        assert_eq!(counts.stopped[0].load(SeqCst), 0);
        assert_eq!(counts.released.load(SeqCst), 0);
        counts.stop_panics.store(false, SeqCst);
        let recovered = cleanup.retry().await;
        assert_eq!(counts.stop_attempts.load(SeqCst), 2);
        assert_eq!(counts.stopped[0].load(SeqCst), 1);
        recovered.unwrap();
        assert_eq!(counts.started[0].load(SeqCst), 1);
        assert_eq!(counts.stop_attempts.load(SeqCst), 2);
        assert_eq!(counts.stopped[0].load(SeqCst), 1);
        assert_eq!(counts.released.load(SeqCst), 1);
        drop(left);
        let _ = runtime.stop().await;
    })
    .await;
}

#[test]
fn explicit_artifact_grants() {
    assert!(
        harmos_sidecar::Attach::observers(&harmos_sidecar::PluginNative::<Twins>::new(std::num::NonZeroUsize::new(2).unwrap()).unwrap())
            .is_empty(),
        "linked observer types are not selected implicitly"
    );
    assert_eq!(MultiStream::STREAMS.len(), 2);
    let source = harmos_sidecar::PluginNative::<MultiStream>::new(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let observers = source.observers();
    assert_eq!(observers.len(), 1);
    assert_eq!(observers[0].streams.len(), 2);

    let empty = harmos_runtime::sidecar::inventory::Declaration::inspect(
        &<Twins as harmos_domain::plugin::Declared>::markers().unwrap().concat(),
        harmos_runtime::sidecar::inventory::Facility::Sidecar,
    )
    .unwrap();
    assert!(empty.observers.is_empty());
    let selected = harmos_runtime::sidecar::inventory::Declaration::inspect(
        &MultiStream::markers().unwrap().concat(),
        harmos_runtime::sidecar::inventory::Facility::Sidecar,
    )
    .unwrap();
    assert_eq!(selected.observers.len(), 1);
    assert_eq!(source.observers(), selected.observers);
    assert_eq!(selected.observers[0].id, observers[0].id);
    assert_eq!(selected.observers[0].interval, observers[0].interval);
    assert_eq!(selected.observers[0].activation, harmos_runtime::sidecar::Activation::Demand);
    assert_eq!(selected.observers[0].streams, MultiStream::STREAMS.iter().map(|entry| entry.id).collect::<Vec<_>>());

    let missing: Vec<_> = MultiStream::markers()
        .unwrap()
        .into_iter()
        .filter(|marker| !String::from_utf8_lossy(marker).contains("\"kind\":\"observers\""))
        .collect();
    assert!(matches!(
        harmos_runtime::sidecar::inventory::Declaration::inspect(&missing.concat(), harmos_runtime::sidecar::inventory::Facility::Sidecar),
        Err(harmos_runtime::sidecar::inventory::DeclarationError::ObserverInventory(_))
    ));
}

/* === Tests === */
