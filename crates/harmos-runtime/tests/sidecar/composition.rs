/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Supervision, Metadata And Teardown Over The In-Process Native Carrier.

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
async fn declared_publication_enters() {
    let mut fixture = operations::Managed::attach(
        meter(Arc::new(AtomicUsize::new(0))),
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(16).unwrap(),
        vec![],
        |resident| resident.ingest::<MeterPulse>(),
    )
    .await;
    fixture.publication().unwrap();
    fixture
        .runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            operations::Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    let runtime = &fixture.runtime;
    fixture.call(PublishPulse { sample: 42 }).await.expect("the sidecar publishes the requested sample");

    let samples: Vec<u64> = runtime
        .streams
        .query::<support::Pulse>(..)
        .await
        .expect("the declared stream answers from its window")
        .map(|row| row.value.sample)
        .collect();
    assert_eq!(samples, [42]);

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn crash_spends_credit() {
    let attachments = Arc::new(AtomicUsize::new(0));
    let source = meter(Arc::clone(&attachments));
    let definition = operations::inspected(&source, serde_json::to_vec(&()).unwrap()).definitions().next().unwrap().clone();
    let calls = Supervised::new(source, (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice));
    let policy = harmos_runtime::operations::OperationPolicy {
        restart: harmos_runtime::operations::RestartPolicy::OnTransient {
            credit: 2,
            sustained_health: Duration::from_secs(30),
            backoff: harmos_runtime::operations::Backoff::Exponential {
                initial: Duration::from_millis(1),
                maximum: Duration::from_millis(2),
                jitter: harmos_runtime::operations::Jitter::Deterministic,
            },
        },
        ..harmos_runtime::services::POLICY
    };
    let runtime = assembly()
        .register(operations::Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_wire_operation::<harmos_runtime::sidecar::Publish<_, _, _>>(
            "meter-publication-core",
            operations::Lanes::NativeOpen,
            Arc::new(calls.clone()),
        )
        .unwrap();
    runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            operations::Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: runtime.streams.clone() }),
        )
        .unwrap();
    let resident = calls.clone();
    runtime
        .operations
        .register_service(harmos_runtime::services::ServiceDefinition { id: "meter".into(), policy }, move |_: Arc<()>| {
            harmos_runtime::sidecar::Managed::new(
                resident.clone(),
                definition.clone(),
                operations::Lanes::NativeOpen,
                NonZeroUsize::new(16).unwrap(),
                NonZeroUsize::new(4096).unwrap(),
            )
            .unwrap()
        })
        .unwrap();
    let lease = runtime.operations.submit_resident("meter").await.unwrap();

    let statuses = runtime.operations.watch_residents();
    service_status(statuses.clone(), "meter", |status| *status == ServiceStatus::Running { credit: 2 }).await;
    let original = calls.operations().unwrap();
    let generation = original.scope().unwrap().generation();
    let outcome = runtime.operations.call_encoded(Collapse::ID, Collapse {}.encode_to_vec()).await;
    let disconnected = match &outcome {
        Err(harmos_runtime::Error::Closing { domain: "operation" | "operation plugin" }) => true,
        Err(harmos_runtime::Error::OperationFailed { operation, source }) if operation == Collapse::ID => {
            matches!(
                source.downcast_ref::<harmos_runtime::operations::NativeError>(),
                Some(harmos_runtime::operations::NativeError::Transport(SidecarError::Unavailable))
            )
        }
        _ => false,
    };
    assert!(disconnected, "the abandoned generation cannot answer: {outcome:?}");

    service_status(statuses.clone(), "meter", |status| *status == ServiceStatus::Running { credit: 1 }).await;
    assert_eq!(attachments.load(Ordering::SeqCst), 2, "a fresh sidecar was attached for the restart");
    assert!(work_facts(&runtime).await.is_empty(), "an environmental restart is silent");

    assert!(
        matches!(original.scope().unwrap().ready().await, Err(harmos_runtime::Error::Closing { domain: "plugin scope" })),
        "the retired generation cannot admit calls"
    );
    assert_ne!(calls.operations().unwrap().scope().unwrap().generation(), generation);
    let amplified = runtime
        .operations
        .call_encoded(Amplify::ID, Amplify { value: 5 }.encode_to_vec())
        .await
        .expect("the restarted sidecar serves the same routes");
    assert_eq!(Amplified::decode(&amplified).unwrap(), Amplified { value: 10 });
    lease.release().await.unwrap();
    runtime.operations.unregister_resident("meter").await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn oversized_exchange_survives() {
    let mut fixture = operations::Managed::attach(
        meter(Arc::new(AtomicUsize::new(0))),
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(16).unwrap(),
        vec![],
        |resident| resident.ingest::<MeterPulse>(),
    )
    .await;
    fixture.publication().unwrap();
    fixture
        .runtime
        .operations
        .register_operation::<Sampling>(
            "meter-sample",
            operations::Lanes::NativeOpen,
            Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
        )
        .unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    let request = BudgetedRoute { payload: vec![0; Frame::LIMIT], reply_bytes: 0, refusal_bytes: 0 };
    assert!(!Frame::request_fits::<BudgetedRoute>(&request));
    let oversized = fixture.call(request).await.unwrap_err();
    assert_transport_refusal(oversized, Frame::OVERSIZED, "request");

    for request in [
        BudgetedRoute { payload: Vec::new(), reply_bytes: Frame::LIMIT as u32, refusal_bytes: 0 },
        BudgetedRoute { payload: Vec::new(), reply_bytes: 0, refusal_bytes: Frame::LIMIT as u32 },
    ] {
        let refused = fixture.call(request).await.unwrap_err();
        assert_transport_refusal(refused, Frame::OVERSIZED, "reply");
    }

    assert_eq!(
        fixture.call(Amplify { value: 5 }).await.unwrap(),
        Amplified { value: 10 },
        "every oversize refusal leaves the same exchange available to a later call"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn announced_exit_reason() {
    let attachments = Arc::new(AtomicUsize::new(0));
    let source = meter(Arc::clone(&attachments));
    let definition = operations::definition(&source, serde_json::to_vec(&()).unwrap());
    let resident = Supervised::new(source, (|_: &Service<Bank>, _: Notice| {}) as fn(&Service<Bank>, Notice));
    let runtime = assembly()
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_wire_operation::<harmos_runtime::sidecar::Publish<_, _, _>>(
            "meter-publication-core",
            HostLanes::Open,
            Arc::new(resident.clone()),
        )
        .unwrap();
    runtime
        .operations
        .register_service(
            harmos_runtime::services::ServiceDefinition { id: "meter".into(), policy: harmos_runtime::services::POLICY },
            move |_: Arc<()>| {
                harmos_runtime::sidecar::Managed::new(
                    resident.clone(),
                    definition.clone(),
                    HostLanes::Open,
                    NonZeroUsize::new(16).unwrap(),
                    NonZeroUsize::new(4096).unwrap(),
                )
                .unwrap()
            },
        )
        .unwrap();
    let _lease = runtime.operations.submit_resident("meter").await.unwrap();
    service_status(runtime.operations.watch_residents(), "meter", |status| matches!(status, ServiceStatus::Running { .. })).await;
    let outcome =
        runtime.operations.call_encoded(Finish::ID, Finish { reason: "the recording is complete".to_owned() }.encode_to_vec()).await;
    assert!(
        matches!(outcome, Err(harmos_runtime::Error::Closing { domain: "operation" | "operation plugin" })),
        "a reply from the closed generation is revoked: {outcome:?}"
    );

    let status =
        service_status(runtime.operations.watch_residents(), "meter", |status| matches!(status, ServiceStatus::Inactive { .. })).await;

    assert!(
        matches!(
            status,
            ServiceStatus::Inactive {
                cause: ServiceExit::Application(ref reason),
                ref history,
            } if reason == "the recording is complete" && history.is_empty()
        ),
        "an announced exit is the sidecar's own decision, not an outage: {status:?}"
    );
    assert_eq!(attachments.load(Ordering::SeqCst), 1, "a sidecar that meant to leave is not started again");

    let facts = work_facts(&runtime).await;
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].name, "deactivated");
    assert!(matches!(runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
}

#[tokio::test]
async fn retired_control_refused() {
    let attached = meter(Arc::new(AtomicUsize::new(0))).attach().await.expect("the in-process sidecar attaches");
    let (write, read, control, _diagnostics, end) = attached.into_parts();
    let harmos_sidecar::transport::Connection { sender: outgoing, receiver: mut incoming, mut driver } =
        harmos_sidecar::transport::Connection::new((read, write), control.expect("the in-process attachment carries a control channel"));
    let driving = tokio::spawn(async move { driver.run().await });

    // Activation Answers On The Same Exchange
    outgoing
        .send(harmos_sidecar::Frame::Call { id: 1, method: harmos_sidecar::Frame::REGISTER.to_owned(), params: Vec::new() })
        .await
        .expect("the activation is sent");
    let answer = received(incoming.receive().await.expect("the exchange remains valid"));
    assert!(matches!(answer, Some(harmos_sidecar::Frame::Answer { id: 1, .. })));

    // The Retired Control Method Is Refused As Unknown
    outgoing
        .send(harmos_sidecar::Frame::Call { id: 2, method: "harmos.initialize".to_owned(), params: b"{}".to_vec() })
        .await
        .expect("the retired control request is sent");
    let refusal = received(incoming.receive().await.expect("the transport remains valid"));
    assert!(matches!(
        refusal,
        Some(harmos_sidecar::Frame::Refusal { id: 2, refusal })
            if refusal.code() == harmos_sidecar::UNKNOWN
    ));

    outgoing.close();
    let _ = driving.await.unwrap();
    drop(outgoing);
    if let Some(mut end) = end {
        end.settle().await.expect("the attachment ending joins");
    }
}

fn received(received: Option<harmos_sidecar::transport::Received>) -> Option<harmos_sidecar::Frame> {
    received.map(|received| {
        if let Some(consumption) = received.consumption {
            consumption.consumed().unwrap();
        }
        received.frame
    })
}

#[tokio::test]
async fn public_private_handlers() {
    let capacity = NonZeroUsize::new(16).unwrap();
    let source = harmos_sidecar::PluginNative::<Gauge>::new(capacity).unwrap();
    let mut fixture = operations::Managed::attach(
        source,
        serde_json::to_vec(&Calibration { factor: 5 }).unwrap(),
        operations::Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;

    assert_eq!(
        fixture.call(visibility::ScalePrivately { value: 4 }).await.unwrap(),
        visibility::Scale { value: 20 },
        "a private method's route is still named and dispatched from outside          the module it was written in"
    );
    assert_eq!(
        fixture.call(visibility::ScalePublicly {}).await.unwrap(),
        visibility::Scale { value: 5 },
        "and a `pub` method's carrier reaches outside the module with it"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn generated_entry_attaches() {
    GAUGE_DEACTIVATED.store(false, Ordering::SeqCst);

    // Bind The Original SDK Owner Before Authored Initialization
    let capacity = NonZeroUsize::new(16).unwrap();
    let source = harmos_sidecar::PluginNative::<Gauge>::new(capacity).unwrap();
    let mut fixture = operations::Managed::attach(
        source,
        serde_json::to_vec(&Calibration { factor: 3 }).unwrap(),
        operations::Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;

    assert_eq!(
        fixture.call(GetMeasure { value: 7 }).await.unwrap(),
        Measured { value: 21 },
        "the route answers under the configuration the entry was activated with"
    );
    let error = fixture.call(GetMeasure { value: -1 }).await.unwrap_err();
    let failure = error.failure().expect("handler refusal preserves its structured failure");
    assert_eq!(failure.code(), harmos_runtime::operations::FailureKind::Permanent);
    assert_eq!(failure.cause(), "this gauge measures no negative value");
    assert_eq!(failure.context().operation(), Some(GetMeasure::ID));
    assert_eq!(
        error.to_string(),
        format!("operation `{}` failed", GetMeasure::ID),
        "the operation frame prints its own line, never the cause"
    );

    // A Nested Transport Failure Arrives As One Cause And One Operation Frame
    let error = fixture.call(GetMeasure { value: i64::MIN }).await.unwrap_err();
    let failure = error.failure().expect("the handler failure crosses as a structured value");
    assert_eq!(failure.cause(), "connection reset");
    assert_eq!(failure.context().operation(), Some(GetMeasure::ID));
    assert_eq!(failure.context().step(), None);
    let diagnostics = failure.diagnostics().expect("origin diagnostics");
    assert_eq!(diagnostics.origin(), harmos_runtime::failure::Origin::Plugin);
    assert_eq!(diagnostics.chain().len(), 3, "the full origin chain stays in diagnostics");
    let rendered = std::iter::successors(Some(&error as &(dyn std::error::Error + 'static)), |error| error.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ");
    assert_eq!(
        rendered,
        format!("operation `{}` failed: connection reset", GetMeasure::ID),
        "no carrier repeats the cause or adds a frame"
    );

    assert_eq!(fixture.call(RunEcho { text: "hello".to_owned() }).await.unwrap(), "hello".to_owned());
    assert_eq!(fixture.call(RunUppercase { text: "hello".to_owned() }).await.unwrap(), "HELLO".to_owned());

    assert_eq!(
        fixture.call(StopSampling {}).await.unwrap(),
        StopSamplingResponse {},
        "a route carrying nothing either way still names one message each way"
    );
    assert_eq!(
        fixture.call(GetFactor {}).await.unwrap(),
        Measured { value: 3 },
        "a reply written through the crate's own alias dispatches like any other"
    );

    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert!(GAUGE_DEACTIVATED.load(Ordering::SeqCst), "the local attachment runs the generated entry's awaited deactivation");
}

#[tokio::test]
async fn refusal_keeps_carrier() {
    // A Publication Outside The Frozen Inventory Is Refused; The Same Carrier Keeps Serving.
    let mut peer = meter(Arc::new(AtomicUsize::new(0)));
    peer.publications = Some(Vec::new());
    let mut fixture = operations::Managed::attach(
        peer,
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(16).unwrap(),
        vec![],
        |resident| resident.ingest::<MeterPulse>(),
    )
    .await;
    fixture.publication().unwrap();
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    assert!(fixture.call(PublishPulse { sample: 42 }).await.is_err());
    assert_eq!(fixture.call(Amplify { value: 5 }).await.unwrap().value, 10);
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn ingestion_authority_bounded() {
    for grants in [
        Vec::new(),
        vec![harmos_sidecar::StreamDeclaration {
            metadata: <MeterPulse as harmos_runtime::StreamSpec>::metadata(),
            row_metadata: Vec::new(),
            id: <MeterPulse as harmos_runtime::StreamSpec>::ID.into(),
            schema: support::Pulse::VERSION + 1,
            window: support::Pulse::WINDOW,
            row: harmos_runtime::schema::graph::Schema::from_declaration(
                <support::Pulse as harmos_runtime::schema::Described>::DEFINITION,
                &harmos_runtime::config::SchemaSettings::default(),
            )
            .unwrap(),
        }],
    ] {
        let mut peer = meter(Arc::new(AtomicUsize::new(0)));
        peer.publications = Some(grants);
        let mut fixture = operations::Managed::attach(
            peer,
            serde_json::to_vec(&()).unwrap(),
            operations::Lanes::NativeOpen,
            NonZeroUsize::new(16).unwrap(),
            vec![],
            |resident| resident.ingest::<MeterPulse>(),
        )
        .await;
        fixture.publication().unwrap();
        fixture
            .runtime
            .operations
            .register_operation::<Sampling>(
                "meter-sample",
                operations::Lanes::NativeOpen,
                Arc::new(SampleProjection { streams: fixture.runtime.streams.clone() }),
            )
            .unwrap();
        fixture.initialize().await.unwrap();
        publications::ready(fixture.ready().clone()).await;
        let refused = fixture.call(PublishPulse { sample: 42 }).await.unwrap_err();
        assert!(matches!(refused, harmos_runtime::Error::OperationFailed { source, .. }
            if matches!(source.downcast_ref::<harmos_runtime::operations::NativeError>(),
                Some(harmos_runtime::operations::NativeError::Remote(failure))
                if failure.cause().contains("stream or schema differs from the frozen generation inventory"))));
        assert_eq!(fixture.runtime.streams.query::<support::Pulse>(..).await.unwrap().count(), 0);
        fixture.shutdown().await.unwrap();
        fixture.runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn graceful_stop_joins() {
    #[derive(Clone)]
    struct Joined<S> {
        source: S,
        ending: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    struct GatedEnding {
        original: harmos_sidecar::attach::Ending,
        ending: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
        joined: bool,
        waiting: Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
        released: bool,
    }
    impl harmos_sidecar::attach::End for GatedEnding {
        fn settle(&mut self) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + '_>> {
            Box::pin(async move {
                if !self.joined {
                    self.original.settle().await?;
                    self.joined = true;
                    self.ending.notify_one();
                }
                if !self.released {
                    if self.waiting.is_none() {
                        let release = self.release.clone();
                        self.waiting = Some(Box::pin(async move { release.notified().await }));
                    }
                    self.waiting.as_mut().unwrap().await;
                    self.waiting.take();
                    self.released = true;
                }
                Ok(())
            })
        }
    }
    impl<S: Attach> Attach for Joined<S> {
        type Error = S::Error;
        fn owner(&self) -> &str {
            self.source.owner()
        }
        fn operations(&self) -> harmos_domain::Result<Vec<harmos_runtime::operations::Definition>> {
            self.source.operations()
        }
        fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_runtime::operations::Contract>>> {
            self.source.operation_contracts()
        }
        fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
            self.source.required_contracts()
        }
        fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[harmos_runtime::operations::lifecycle::Descriptor; 2]>> {
            self.source.sdk_lifecycle()
        }
        fn publications(&self) -> Vec<harmos_sidecar::StreamDeclaration> {
            self.source.publications()
        }
        fn observers(&self) -> Vec<harmos_sidecar::inventory::observers::Declaration> {
            self.source.observers()
        }
        fn observe_contract(&self) -> harmos_domain::Result<Option<harmos_runtime::operations::Contract>> {
            self.source.observe_contract()
        }
        fn listener_declarations(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
            self.source.listener_declarations()
        }
        async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
            let (write, read, control, diagnostics, original) = self.source.attach().await?.into_parts();
            let receipt = harmos_sidecar::attach::Ending::new(GatedEnding {
                original: original.expect("the SDK retains its original physical ending"),
                ending: self.ending.clone(),
                release: self.release.clone(),
                joined: false,
                waiting: None,
                released: false,
            });
            let mut attached = harmos_sidecar::Attached::new(write, read).ending(receipt);
            if let Some(control) = control {
                attached = attached.control(control);
            }
            if let Some(diagnostics) = diagnostics {
                attached = attached.diagnostics(diagnostics);
            }
            Ok(attached)
        }
    }

    let ending = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let capacity = NonZeroUsize::new(1).unwrap();
    let source = Joined {
        source: harmos_sidecar::PluginNative::<Joinable>::new(capacity).unwrap(),
        ending: ending.clone(),
        release: release.clone(),
    };
    let mut fixture = operations::Managed::attach(
        source,
        serde_json::to_vec(&()).unwrap(),
        operations::Lanes::NativeOpen,
        capacity,
        vec![],
        std::convert::identity,
    )
    .await;
    fixture.initialize().await.unwrap();
    publications::ready(fixture.ready().clone()).await;
    {
        let stop = fixture.shutdown();
        tokio::pin!(stop);
        tokio::select! {
            result = &mut stop => panic!("shutdown returned without joining its attachment: {result:?}"),
            _ = ending.notified() => {}
        }
        // Cancel The Borrow While The Original Ending Retains Its Gate Wait.
    }
    release.notify_one();
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
}

/* === Tests === */

fn assert_transport_refusal(error: harmos_runtime::Error, code: i64, detail: &str) {
    let harmos_runtime::Error::OperationFailed { source, .. } = error else {
        panic!("transport refusal preserves its operation cause: {error}")
    };
    assert!(
        matches!(source.downcast_ref::<harmos_runtime::operations::NativeError>(),
        Some(harmos_runtime::operations::NativeError::Transport(SidecarError::Refused { code: actual, failure }))
            if *actual == code && failure.cause().contains(detail)),
        "{source}"
    );
}

async fn work_facts(runtime: &Runtime<Bank>) -> Vec<SystemFact> {
    runtime
        .state
        .query::<SystemFact>(Position::ORIGIN)
        .await
        .filter_map(Result::ok)
        .map(|entry| entry.value)
        .filter(|fact| matches!(fact.source, FactSource::Work(_)))
        .collect()
}
