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

use crate::operation_component_contract as guest_contract;
use crate::process_core_contract as contract;
use crate::support::fixtures as prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use super::{HostLanes, assembly, process_definition, service_status};
use crate::core_egress::Endpoint;
use crate::support::Bank;
use contract::{Inspect, Queries};
use harmos::plugin::sidecar::{Executable, Notice, Supervised};
use harmos::{Runtime, services::Service, services::ServicePolicy, services::ServiceStatus};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct DeadlineRead;
struct Fixture {
    runtime: Runtime<Bank>,
    _lease: harmos::services::ServiceLease<Bank>,
    root: tempfile::TempDir,
    prepared: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

/* === Enums === */
#[harmos::catalog]
enum EmbeddedEgress {
    Fetch(crate::core_egress::Egress),
}

#[harmos::catalog]
enum GuestOperations {
    Read(guest_contract::Read),
    Counts(guest_contract::Counts),
    Upload(guest_contract::Upload),
    Download(guest_contract::Download),
    Exchange(guest_contract::Exchange),
}

/* === Traits === */

/* === Implementations === */

impl harmos::operations::OperationDeclaration for DeadlineRead {
    type Request = String;
    type Response = String;
    const ID: &'static str = "fixture.context.deadline";
    const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
        timeout: Duration::from_millis(400),
        attempt_timeout: Duration::from_millis(300),
        retries: harmos::operations::RetryPolicy::Never,
    };
}
impl harmos::operations::Operation for DeadlineRead {
    type Declaration = Self;
    type Resources = ();
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = harmos::operations::Context<'a>;
    fn new(_: Arc<()>) -> harmos::Result<Self> {
        Ok(Self)
    }
    async fn run<'a>(
        &mut self,
        context: Self::Context<'a>,
        request: &mut String,
        _: &mut Self::Endpoints,
    ) -> harmos::Result<harmos::operations::Response<String>> {
        let result: ::core::result::Result<String, Self::Error> = async move {
            let child = context.with_value(contract::Stage {
                id: "parent".into(),
            })?;
            let answer = child.call::<Inspect>(request.clone()).await?;
            Ok((*answer).clone())
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}

impl Fixture {
    async fn new(provided: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let source =
            Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture")).unwrap();
        let mut definition = process_definition(&source);
        definition.configuration = serde_json::to_vec(root.path().to_str().unwrap()).unwrap();
        let encoding_bytes = definition.configuration.len() * 2
            + std::mem::size_of::<harmos::operations::lifecycle::InitializeRequest>()
            + 84;
        assert!(encoding_bytes <= 512);
        let prepared = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let runtime = assembly()
            .operation_settings(harmos::operations::Settings {
                preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                ..Default::default()
            })
            .runtime()
            .await
            .unwrap();
        if provided {
            runtime
                .operations
                .register_wire_operation::<crate::core_egress::Egress>(
                    "native-egress",
                    HostLanes::Open,
                    Arc::new(Endpoint {
                        answering: Duration::ZERO,
                        calls: calls.clone(),
                        prepared: prepared.clone(),
                    }),
                )
                .unwrap();
        }
        let resident = Supervised::new(source, notices as fn(&Service<Bank>, Notice))
            .grace(Duration::from_millis(50));
        runtime
            .services
            .register(
                harmos::services::ServiceDefinition {
                    id: "native-core".into(),
                    policy: ServicePolicy {
                        credit: 0,
                        ..Default::default()
                    },
                },
                move |_: Arc<()>| {
                    harmos::plugin::sidecar::Managed::new_executable_catalog::<Queries>(
                        resident.clone(),
                        definition.clone(),
                        HostLanes::Open,
                        NonZeroUsize::new(2).unwrap(),
                        NonZeroUsize::new(encoding_bytes).unwrap(),
                    )
                    .unwrap()
                },
            )
            .unwrap();
        let lease = runtime
            .services
            .activate("native-core", &runtime.services.root_scope())
            .await
            .unwrap();
        Self {
            runtime,
            _lease: lease,
            root,
            prepared,
            calls,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn notices(_: &Service<Bank>, _: Notice) {}

pub(super) fn contract_scenario() {
    scenario_runtime::check("native_child_overlay_keeps_authority", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let f = Fixture::new(true).await;
            service_status(f.runtime.services.subscribe(), "native-core", |status| {
                matches!(status, ServiceStatus::Running { .. })
            })
            .await;
            let mut context = harmos::operations::CallContext::default();
            context.insert(73u64).unwrap();
            context
                .insert_portable(contract::Stage {
                    id: "parent".into(),
                })
                .unwrap();
            let caller = f.runtime.operations.with_context(context.clone());
            let answer = caller.call::<Inspect>("context".into()).await.unwrap();
            assert_eq!(&*answer, "parent:child");
            assert_eq!(context.require::<contract::Stage>().unwrap().id, "parent");
            assert_eq!(f.calls.load(Ordering::SeqCst), 1);
            let refused = caller
                .call::<Inspect>("context-denied".into())
                .await
                .unwrap();
            assert!(refused.contains("may not invoke operation"), "{refused}");
            assert!(refused.contains("host.egress"), "{refused}");
            assert_eq!(f.calls.load(Ordering::SeqCst), 1);
            f.runtime.stop().await.unwrap();
        });
    });
    scenario_runtime::check("native_child_overlay_cancellation_and_deadline", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            for deadline in [false, true] {
                let f = Fixture::new(true).await;
                service_status(f.runtime.services.subscribe(), "native-core", |status| {
                    matches!(status, ServiceStatus::Running { .. })
                })
                .await;
                f.runtime
                    .operations
                    .register_operation::<DeadlineRead>("fixture", HostLanes::Open, Arc::new(()))
                    .unwrap();
                let mut context = harmos::operations::CallContext::default();
                context.insert(73u64).unwrap();
                context
                    .insert_portable(contract::Stage {
                        id: "parent".into(),
                    })
                    .unwrap();
                let caller = f.runtime.operations.with_context(context);
                if deadline {
                    let result = tokio::time::timeout(
                        Duration::from_secs(3),
                        caller.call::<DeadlineRead>("context-wait".into()),
                    )
                    .await
                    .unwrap();
                    assert!(
                        matches!(
                            result,
                            Err(harmos::Error::OperationRefused(harmos::Refusal::TimedOut))
                        ),
                        "{result:?}"
                    );
                } else {
                    let mut invocation = caller.submit::<Inspect>("context-wait".into()).unwrap();
                    tokio::time::timeout(Duration::from_secs(2), async {
                        while f.calls.load(Ordering::SeqCst) == 0 {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    })
                    .await
                    .unwrap();
                    invocation.cancel();
                    let result = tokio::time::timeout(Duration::from_secs(3), &mut invocation)
                        .await
                        .unwrap();
                    assert!(
                        matches!(
                            result,
                            Err(harmos::Error::OperationRefused(harmos::Refusal::Cancelled))
                        ),
                        "{result:?}"
                    );
                    assert_eq!(
                        invocation.status().phase(),
                        harmos::operations::OperationPhase::Settled
                    );
                }
                assert_eq!(f.calls.load(Ordering::SeqCst), 1);
                tokio::time::timeout(Duration::from_secs(3), f.runtime.stop())
                    .await
                    .unwrap()
                    .unwrap();
            }
        });
    });
    scenario_runtime::check("cleanup_retry_preserves_attempt", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let root = tempfile::tempdir().unwrap();
            let source = Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture"))
                .unwrap()
                .variable("HARMOS_TEST_CORE_FAIL_SHUTDOWN", "1");
            let owner = source.declaration().id.clone();
            let runtime = assembly()
                .operation_settings(harmos::config::OperationSettings {
                    preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                    ..Default::default()
                })
                .runtime()
                .await
                .unwrap();
            runtime
                .operations
                .register_wire_operation::<crate::core_egress::Egress>(
                    "native-egress",
                    HostLanes::Open,
                    Arc::new(Endpoint {
                        answering: Duration::ZERO,
                        calls: Arc::new(AtomicUsize::new(0)),
                        prepared: Arc::new(AtomicUsize::new(0)),
                    }),
                )
                .unwrap();
            runtime
                .plugin
                .register_sidecar::<Queries>(
                    source,
                    harmos::config::PluginSidecarSettings {
                        plugin: harmos::config::PluginSettings {
                            configuration: serde_json::to_vec(root.path().to_str().unwrap())
                                .unwrap(),
                            host_operations: vec![harmos::Identifier::new("host.egress").unwrap()],
                            restart: harmos::config::PluginRestartSettings {
                                retries: 2,
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    HostLanes::Open,
                )
                .await
                .unwrap();
            runtime.plugin.initialize().await.unwrap();
            let original = runtime.plugin.status(&owner).unwrap().generation;
            let error = runtime.plugin.stop(&owner).await.unwrap_err();
            let harmos::Error::PluginLifecycle { source } = &error else {
                panic!("expected retained plugin cleanup: {error:?}");
            };
            let harmos::plugin::PluginFailure::Cleanup(failures) = source.as_ref() else {
                panic!("expected retained plugin cleanup: {source:?}");
            };
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].artifact, owner.as_str());
            let mut cause: &dyn std::error::Error = &failures[0].error;
            while let Some(source) = cause.source() {
                cause = source;
            }
            assert!(cause.to_string().contains("controlled shutdown failure"));
            let failed = runtime.plugin.status(&owner).unwrap();
            assert_eq!(failed.attempts, 1);
            assert_eq!(failed.generation, original);
            assert!(failed.cleanup_pending);
            assert!(
                failed
                    .last_failure
                    .as_ref()
                    .unwrap()
                    .contains("process-core.sdk.shutdown"),
                "retained failure: {:?}",
                failed.last_failure
            );
            assert!(failed.next_retry.is_none());
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert_eq!(std::fs::read(root.path().join("prepared")).unwrap(), b"1\n");
            runtime.plugin.stop(&owner).await.unwrap();
            let stopped = runtime.plugin.status(&owner).unwrap();
            assert_eq!(stopped.attempts, 1);
            assert!(!stopped.cleanup_pending);
            assert!(stopped.generation.is_none());
            assert!(stopped.next_retry.is_none());
            for marker in ["prepared", "initialized", "shutdown", "cleaned"] {
                assert_eq!(std::fs::read(root.path().join(marker)).unwrap(), b"1\n");
            }
            runtime.stop().await.unwrap();
        });
    });
    scenario_runtime::check("runtime_crashes_spend_credit", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let root = tempfile::tempdir().unwrap();
            let source =
                Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture")).unwrap();
            let owner = source.declaration().id.clone();
            let calls = Arc::new(AtomicUsize::new(0));
            let runtime = assembly()
                .operation_settings(harmos::config::OperationSettings {
                    preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                    ..Default::default()
                })
                .runtime()
                .await
                .unwrap();
            runtime
                .operations
                .register_wire_operation::<crate::core_egress::Egress>(
                    "native-egress",
                    HostLanes::Open,
                    Arc::new(Endpoint {
                        answering: Duration::ZERO,
                        calls: calls.clone(),
                        prepared: Arc::new(AtomicUsize::new(0)),
                    }),
                )
                .unwrap();
            runtime
                .plugin
                .register_sidecar::<Queries>(
                    source,
                    harmos::config::PluginSidecarSettings {
                        plugin: harmos::config::PluginSettings {
                            configuration: serde_json::to_vec(root.path().to_str().unwrap())
                                .unwrap(),
                            host_operations: vec![harmos::Identifier::new("host.egress").unwrap()],
                            restart: harmos::config::PluginRestartSettings {
                                retries: 2,
                                backoff_base: Duration::from_millis(50),
                                backoff_cap: Duration::from_millis(100),
                            },
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    HostLanes::Open,
                )
                .await
                .unwrap();
            runtime.plugin.initialize().await.unwrap();
            assert_eq!(
                runtime
                    .operations
                    .call::<Inspect>("vendor".into())
                    .await
                    .unwrap()
                    .as_str(),
                "egress status 200"
            );
            for attempt in 1..=3 {
                let generation = runtime.plugin.status(&owner).unwrap().generation.unwrap();
                let crashed = tokio::time::timeout(
                    Duration::from_secs(5),
                    runtime.operations.call::<Inspect>("crash".into()),
                )
                .await;
                assert!(
                    crashed.is_ok(),
                    "crash receipt stalled: {:?}",
                    runtime.plugin.status(&owner)
                );
                assert!(crashed.unwrap().is_err());
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let status = runtime.plugin.status(&owner).unwrap();
                        let ready = attempt < 3
                            && status.state == harmos::plugin::PluginState::Ready
                            && status.attempts == attempt + 1;
                        let exhausted = attempt == 3
                            && status.state == harmos::plugin::PluginState::Failed
                            && status.attempts == 3
                            && status.next_retry.is_none();
                        if ready || exhausted {
                            assert_ne!(status.generation, Some(generation));
                            assert!(!status.cleanup_pending);
                            assert_eq!(
                                status.failure_class,
                                Some(harmos::plugin::PluginFailureClass::Recoverable)
                            );
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
                assert_eq!(
                    calls.load(Ordering::SeqCst),
                    1,
                    "an uncertain call is never replayed during restart"
                );
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
            assert_eq!(runtime.plugin.status(&owner).unwrap().attempts, 3);
            runtime.stop().await.unwrap();
        });
    });
    scenario_runtime::check("native_restart_policy", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            for (name, crashes, retries, stop, invalid) in [
                ("recovers", 1, 2, false, false),
                ("exhausts", 10, 2, false, false),
                ("stops", 10, 2, true, false),
                ("invalid", 0, 2, false, true),
            ] {
                let root = tempfile::tempdir().unwrap();
                let source =
                    Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture"))
                        .unwrap()
                        .variable("HARMOS_TEST_CORE_CRASH_STARTS", crashes.to_string());
                let owner = source.declaration().id.clone();
                let runtime = assembly()
                    .operation_settings(harmos::config::OperationSettings {
                        preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                        ..Default::default()
                    })
                    .runtime()
                    .await
                    .unwrap();
                runtime
                    .operations
                    .register_wire_operation::<crate::core_egress::Egress>(
                        "native-egress",
                        HostLanes::Open,
                        Arc::new(Endpoint {
                            answering: Duration::ZERO,
                            calls: Arc::new(AtomicUsize::new(0)),
                            prepared: Arc::new(AtomicUsize::new(0)),
                        }),
                    )
                    .unwrap();
                let configuration = if invalid {
                    b"{}".to_vec()
                } else {
                    serde_json::to_vec(root.path().to_str().unwrap()).unwrap()
                };
                runtime
                    .plugin
                    .register_sidecar::<Queries>(
                        source,
                        harmos::config::PluginSidecarSettings {
                            plugin: harmos::config::PluginSettings {
                                configuration,
                                host_operations: vec![
                                    harmos::Identifier::new("host.egress").unwrap(),
                                ],
                                restart: harmos::config::PluginRestartSettings {
                                    retries,
                                    backoff_base: Duration::from_millis(150),
                                    backoff_cap: Duration::from_millis(150),
                                },
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        HostLanes::Open,
                    )
                    .await
                    .unwrap();
                assert!(runtime.plugin.initialize().await.is_err(), "{name}");
                let failed = runtime.plugin.status(&owner).unwrap();
                assert_eq!(failed.attempts, 1, "{name}");
                assert!(!failed.cleanup_pending, "{name}");
                if invalid {
                    assert_eq!(
                        failed.failure_class,
                        Some(harmos::plugin::PluginFailureClass::Terminal)
                    );
                    assert!(failed.next_retry.is_none());
                } else {
                    assert_eq!(
                        failed.failure_class,
                        Some(harmos::plugin::PluginFailureClass::Recoverable),
                        "{failed:?}"
                    );
                    assert!(failed.next_retry.is_some());
                }
                if stop {
                    runtime.plugin.stop(&owner).await.unwrap();
                }
                if !stop && !invalid {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        loop {
                            let status = runtime.plugin.status(&owner).unwrap();
                            if (crashes == 1 && status.state == harmos::plugin::PluginState::Ready)
                                || (crashes > 1
                                    && status.attempts == 3
                                    && status.next_retry.is_none()
                                    && status.state == harmos::plugin::PluginState::Failed)
                            {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .unwrap();
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
                let settled = runtime.plugin.status(&owner).unwrap();
                let expected = if stop || invalid {
                    1
                } else if crashes == 1 {
                    2
                } else {
                    3
                };
                assert_eq!(settled.attempts, expected, "{name}");
                assert!(settled.next_retry.is_none(), "{name}");
                assert!(!settled.cleanup_pending, "{name}");
                runtime.stop().await.unwrap();
                if crashes == 1 {
                    for marker in ["initialized", "shutdown", "cleaned"] {
                        assert_eq!(std::fs::read(root.path().join(marker)).unwrap(), b"1\n");
                    }
                }
            }
        });
    });
    scenario_runtime::check("shared_backends_dependency_grants_and_reload", || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let root = tempfile::tempdir().unwrap();
                let source =
                    Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture"))
                        .unwrap();
                let owner = source.declaration().id.clone();
                let runtime = assembly()
                    .operation_settings(harmos::config::OperationSettings {
                        preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                        ..Default::default()
                    })
                    .runtime()
                    .await
                    .unwrap();
                let settings = harmos::config::PluginSidecarSettings {
                    plugin: harmos::config::PluginSettings {
                        configuration: serde_json::to_vec(root.path().to_str().unwrap()).unwrap(),
                        requires: vec![harmos::plugin::Requirement {
                            id: "embedded-egress".into(),
                            version: "*".parse().unwrap(),
                        }],
                        // The Exact SDK Requirement Selects A Plugin, Not A Host Grant.
                        host_operations: vec![],
                        ..Default::default()
                    },
                    ..Default::default()
                };
                assert!(
                    runtime
                        .plugin
                        .register_sidecar::<Queries>(
                            source.clone(),
                            settings.clone(),
                            HostLanes::Open,
                        )
                        .await
                        .is_err()
                );
                assert!(runtime.plugin.status(&owner).is_none());
                assert!(!root.path().join("prepared").exists());
                let calls = Arc::new(AtomicUsize::new(0));
                let prepared = Arc::new(AtomicUsize::new(0));
                runtime
                    .plugin
                    .register_embedded_wire::<EmbeddedEgress, _>(
                        harmos::plugin::PluginDefinition {
                            id: "embedded-egress".into(),
                            version: "1.0.0".parse().unwrap(),
                            artifact: harmos::plugin::PluginArtifact::Embedded {
                                revision: "egress".into(),
                            },
                            requires: vec![],
                            configuration: b"null".to_vec(),
                            contributions: Default::default(),
                        },
                        Default::default(),
                        HostLanes::Open,
                        Arc::new(Endpoint {
                            answering: Duration::from_millis(150),
                            calls: calls.clone(),
                            prepared: prepared.clone(),
                        }),
                    )
                    .await
                    .unwrap();
                runtime
                    .plugin
                    .register_sidecar::<Queries>(source, settings, HostLanes::Open)
                    .await
                    .unwrap();
                let component = prepared_fixtures::component(
                    "operation-component",
                    if cfg!(feature = "grpc") {
                        "componentize,kit,grpc,streaming"
                    } else {
                        "componentize,kit,streaming"
                    },
                    None,
                );
                let guest = harmos::plugin::guest::Host::new(&runtime).unwrap();
                runtime
                    .plugin
                    .register_guest::<GuestOperations>(
                        guest.load(&component).unwrap(),
                        harmos::config::PluginSettings {
                            configuration: serde_json::to_vec("normal").unwrap(),
                            ..Default::default()
                        },
                        HostLanes::Open,
                    )
                    .await
                    .unwrap();
                for id in ["embedded-egress", owner.as_str(), "operation-guest"] {
                    assert_eq!(runtime.plugin.status(id).unwrap().attempts, 0);
                }
                runtime.plugin.initialize().await.unwrap();
                let guest_generation = runtime.plugin.status("operation-guest").unwrap().generation;
                let mut context = harmos::operations::CallContext::default();
                context
                    .insert_portable(guest_contract::Stage {
                        id: "parent".into(),
                    })
                    .unwrap();
                let answer = runtime
                    .operations
                    .with_context(context)
                    .call::<guest_contract::Read>(guest_contract::Value {
                        value: "context-parent".into(),
                    })
                    .await
                    .unwrap();
                assert_eq!(answer.value, "parent:child");
                let provider = runtime.plugin.status("embedded-egress").unwrap().generation;
                let consumer = runtime.plugin.status(&owner).unwrap().generation;
                let denied = runtime
                    .operations
                    .call::<Inspect>("other".into())
                    .await
                    .unwrap();
                assert!(denied.contains("may not invoke operation"), "{denied}");
                assert!(denied.contains("host.egress"), "{denied}");
                assert_eq!(calls.load(Ordering::SeqCst), 0);
                assert_eq!(prepared.load(Ordering::SeqCst), 0);
                let invoking = {
                    let operations = runtime.operations.clone();
                    tokio::spawn(async move { operations.call::<Inspect>("vendor".into()).await })
                };
                tokio::time::timeout(Duration::from_secs(5), async {
                    while calls.load(Ordering::SeqCst) == 0 {
                        assert!(!invoking.is_finished());
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
                assert!(!invoking.is_finished());
                tokio::time::timeout(
                    Duration::from_secs(5),
                    runtime.plugin.reload("embedded-egress"),
                )
                .await
                .unwrap()
                .unwrap();
                // Retirement Joins The Original Carrier And Its In-Flight Receipt.
                let _outcome = tokio::time::timeout(Duration::from_secs(1), invoking)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert_eq!(prepared.load(Ordering::SeqCst), 1);
                for (id, previous) in [("embedded-egress", provider), (owner.as_str(), consumer)] {
                    let ready = runtime.plugin.status(id).unwrap();
                    assert_eq!(ready.attempts, 2);
                    assert_ne!(ready.generation, previous);
                    assert_eq!(ready.state, harmos::plugin::PluginState::Ready);
                    assert!(!ready.cleanup_pending);
                }
                let unchanged = runtime.plugin.status("operation-guest").unwrap();
                assert_eq!(unchanged.generation, guest_generation);
                assert_eq!(unchanged.attempts, 1);
                assert_eq!(unchanged.state, harmos::plugin::PluginState::Ready);
                for marker in ["prepared", "initialized"] {
                    assert_eq!(std::fs::read(root.path().join(marker)).unwrap(), b"1\n1\n");
                }
                for marker in ["shutdown", "cleaned"] {
                    assert_eq!(std::fs::read(root.path().join(marker)).unwrap(), b"1\n");
                }
                assert_eq!(
                    runtime
                        .operations
                        .call::<Inspect>("vendor".into())
                        .await
                        .unwrap()
                        .as_str(),
                    "egress status 200"
                );
                assert_eq!(calls.load(Ordering::SeqCst), 2);
                runtime.plugin.stop("embedded-egress").await.unwrap();
                for id in ["embedded-egress", owner.as_str()] {
                    let stopped = runtime.plugin.status(id).unwrap();
                    assert!(stopped.generation.is_none());
                    assert_eq!(stopped.attempts, 2);
                    assert!(!stopped.cleanup_pending);
                }
                assert!(
                    runtime
                        .operations
                        .call::<Inspect>("vendor".into())
                        .await
                        .is_err()
                );
                let unchanged = runtime.plugin.status("operation-guest").unwrap();
                assert_eq!(unchanged.generation, guest_generation);
                assert_eq!(unchanged.attempts, 1);
                assert_eq!(
                    runtime
                        .operations
                        .call::<guest_contract::Read>(guest_contract::Value {
                            value: "leaf".into()
                        })
                        .await
                        .unwrap()
                        .value,
                    "leaf:1"
                );
                runtime.stop().await.unwrap();
                for id in ["embedded-egress", owner.as_str(), "operation-guest"] {
                    let stopped = runtime.plugin.status(id).unwrap();
                    assert!(stopped.generation.is_none());
                    assert!(!stopped.cleanup_pending);
                }
            });
    });
    scenario_runtime::check("native_reload_interruption", || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                for replace in [false, true] {
                    for interruption in ["caller", "stop", "closing"] {
                        let root = tempfile::tempdir().unwrap();
                        let replacement_root = tempfile::tempdir().unwrap();
                        let source = Executable::at(prepared_fixtures::artifact(
                            "harmos-process-core-fixture",
                        ))
                        .unwrap();
                        let owner = source.declaration().id.clone();
                        let runtime = assembly()
                            .operation_settings(harmos::config::OperationSettings {
                                preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                                ..Default::default()
                            })
                            .runtime()
                            .await
                            .unwrap();
                        let runtime = Arc::new(runtime);
                        runtime
                            .operations
                            .register_wire_operation::<crate::core_egress::Egress>(
                                "native-egress",
                                HostLanes::Open,
                                Arc::new(Endpoint {
                                    answering: Duration::ZERO,
                                    calls: Arc::new(AtomicUsize::new(0)),
                                    prepared: Arc::new(AtomicUsize::new(0)),
                                }),
                            )
                            .unwrap();
                        let settings =
                            |directory: &std::path::Path| harmos::config::PluginSidecarSettings {
                                plugin: harmos::config::PluginSettings {
                                    configuration: serde_json::to_vec(directory.to_str().unwrap())
                                        .unwrap(),
                                    host_operations: vec![
                                        harmos::Identifier::new("host.egress").unwrap(),
                                    ],
                                    ..Default::default()
                                },
                                ..Default::default()
                            };
                        runtime
                            .plugin
                            .register_sidecar::<Queries>(
                                source,
                                settings(root.path()),
                                HostLanes::Open,
                            )
                            .await
                            .unwrap();
                        runtime.plugin.initialize().await.unwrap();
                        let original = runtime.plugin.status(&owner).unwrap().generation;
                        let active_root = if replace {
                            replacement_root.path()
                        } else {
                            root.path()
                        };
                        std::fs::write(active_root.join("hold-initialize"), []).unwrap();
                        let handle = runtime.handle();
                        let reloading = {
                            let plugin = handle.plugin.clone();
                            let owner = owner.clone();
                            let replacement = Executable::at(prepared_fixtures::artifact(
                                "harmos-process-core-fixture",
                            ))
                            .unwrap();
                            let settings = settings(active_root);
                            tokio::spawn(async move {
                                if replace {
                                    plugin
                                        .reload_sidecar::<Queries>(
                                            replacement,
                                            settings,
                                            HostLanes::Open,
                                        )
                                        .await
                                } else {
                                    plugin.reload(&owner).await
                                }
                            })
                        };
                        tokio::time::timeout(Duration::from_secs(5), async {
                            while !active_root.join("initializing").exists() {
                                assert!(!reloading.is_finished());
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        })
                        .await
                        .unwrap();
                        let current = handle.plugin.status(&owner).unwrap();
                        assert_eq!(current.attempts, 2);
                        assert_ne!(current.generation, original);
                        let mut reloading = Some(reloading);
                        let stopping = match interruption {
                            "caller" => {
                                let reloading = reloading.take().unwrap();
                                reloading.abort();
                                assert!(reloading.await.unwrap_err().is_cancelled());
                                None
                            }
                            "stop" => {
                                let plugin = handle.plugin.clone();
                                let owner = owner.clone();
                                Some(tokio::spawn(async move { plugin.stop(&owner).await }))
                            }
                            "closing" => {
                                let runtime = runtime.clone();
                                Some(tokio::spawn(
                                    async move { runtime.stop().await.map(|_| ()) },
                                ))
                            }
                            _ => unreachable!(),
                        };
                        // Observe The Original Retirement Owner Before Releasing Initialization.
                        tokio::time::timeout(Duration::from_secs(1), async {
                            loop {
                                let status = handle.plugin.status(&owner).unwrap();
                                if status.state == harmos::plugin::PluginState::Stopping {
                                    assert_eq!(status.generation, current.generation);
                                    assert_eq!(status.attempts, 2);
                                    assert!(status.cleanup_pending);
                                    break;
                                }
                                if status.generation.is_none() {
                                    assert!(!status.cleanup_pending);
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        })
                        .await
                        .unwrap();
                        assert!(
                            handle
                                .operations
                                .call::<Inspect>("vendor".into())
                                .await
                                .is_err()
                        );
                        std::fs::remove_file(active_root.join("hold-initialize")).unwrap();
                        if let Some(stopping) = stopping {
                            tokio::time::timeout(Duration::from_secs(5), stopping)
                                .await
                                .unwrap()
                                .unwrap()
                                .unwrap();
                        } else {
                            tokio::time::timeout(Duration::from_secs(5), async {
                                while handle.plugin.status(&owner).unwrap().generation.is_some() {
                                    tokio::time::sleep(Duration::from_millis(5)).await;
                                }
                            })
                            .await
                            .unwrap();
                        }
                        if let Some(reloading) = reloading {
                            assert!(reloading.await.unwrap().is_err());
                        }
                        let settled = handle.plugin.status(&owner).unwrap();
                        assert_eq!(settled.attempts, 2);
                        assert!(settled.generation.is_none());
                        assert!(!settled.cleanup_pending);
                        assert!(settled.next_retry.is_none());
                        if interruption != "closing" {
                            assert_eq!(
                                settled.failure_class,
                                Some(harmos::plugin::PluginFailureClass::Cancelled)
                            );
                        }
                        if interruption == "closing" {
                            continue;
                        }
                        // Explicit Reactivation Uses The Committed Replacement Factory.
                        handle.plugin.reload(&owner).await.unwrap();
                        assert_eq!(handle.plugin.status(&owner).unwrap().attempts, 3);
                        assert_eq!(
                            std::fs::read(active_root.join("prepared")).unwrap(),
                            if replace {
                                b"1\n1\n".as_slice()
                            } else {
                                b"1\n1\n1\n".as_slice()
                            }
                        );
                        handle.plugin.stop(&owner).await.unwrap();
                        runtime.stop().await.unwrap();
                    }
                }
            });
    });
    scenario_runtime::check("runtime_owned_native_plugin", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let root = tempfile::tempdir().unwrap();
            let source =
                Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture")).unwrap();
            let prepared = Arc::new(AtomicUsize::new(0));
            let calls = Arc::new(AtomicUsize::new(0));
            let runtime = assembly()
                .operation_settings(harmos::config::OperationSettings {
                    preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
                    ..Default::default()
                })
                .runtime()
                .await
                .unwrap();
            runtime
                .operations
                .register_wire_operation::<crate::core_egress::Egress>(
                    "native-egress",
                    HostLanes::Open,
                    Arc::new(Endpoint {
                        answering: Duration::ZERO,
                        calls: calls.clone(),
                        prepared,
                    }),
                )
                .unwrap();
            let owner = source.declaration().id.clone();
            let settings = harmos::config::PluginSidecarSettings {
                plugin: harmos::config::PluginSettings {
                    configuration: serde_json::to_vec(root.path().to_str().unwrap()).unwrap(),
                    host_operations: vec![harmos::Identifier::new("host.egress").unwrap()],
                    ..Default::default()
                },
                ..Default::default()
            };
            runtime
                .plugin
                .register_sidecar::<Queries>(source, settings, HostLanes::Open)
                .await
                .unwrap();
            assert_eq!(runtime.plugin.status(&owner).unwrap().attempts, 0);
            assert!(!root.path().join("prepared").exists());
            tokio::time::timeout(Duration::from_secs(5), runtime.plugin.initialize())
                .await
                .unwrap()
                .unwrap();
            let handle = runtime.handle();
            assert!(runtime.plugin.same_instance(&handle.plugin));
            assert_eq!(
                handle.plugin.status(&owner).unwrap().state,
                harmos::plugin::PluginState::Ready
            );
            let answer = runtime
                .operations
                .call::<Inspect>("vendor".into())
                .await
                .unwrap();
            assert_eq!(answer.as_str(), "egress status 200");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            let replacement_root = tempfile::tempdir().unwrap();
            let replacement =
                Executable::at(prepared_fixtures::artifact("harmos-process-core-fixture")).unwrap();
            runtime
                .plugin
                .reload_sidecar::<Queries>(
                    replacement,
                    harmos::config::PluginSidecarSettings {
                        plugin: harmos::config::PluginSettings {
                            configuration: serde_json::to_vec(
                                replacement_root.path().to_str().unwrap(),
                            )
                            .unwrap(),
                            host_operations: vec![harmos::Identifier::new("host.egress").unwrap()],
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    HostLanes::Open,
                )
                .await
                .unwrap();
            assert_eq!(runtime.plugin.status(&owner).unwrap().attempts, 2);
            for marker in ["prepared", "initialized", "shutdown", "cleaned"] {
                assert_eq!(std::fs::read(root.path().join(marker)).unwrap(), b"1\n");
            }
            assert_eq!(
                runtime
                    .operations
                    .call::<Inspect>("vendor".into())
                    .await
                    .unwrap()
                    .as_str(),
                "egress status 200"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            tokio::time::timeout(Duration::from_secs(5), runtime.stop())
                .await
                .unwrap()
                .unwrap();
            for marker in ["prepared", "initialized", "shutdown", "cleaned"] {
                assert_eq!(std::fs::read(root.path().join(marker)).unwrap(), b"1\n");
            }
            for marker in ["prepared", "initialized", "shutdown", "cleaned"] {
                assert_eq!(
                    std::fs::read(replacement_root.path().join(marker)).unwrap(),
                    b"1\n"
                );
            }
            assert!(handle.plugin.status(&owner).unwrap().generation.is_none());
        });
    });
    {
        scenario_runtime::check("egress_grant_checked", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(true).await;
                    service_status(
                        fixture.runtime.services.subscribe(),
                        "native-core",
                        |status| matches!(status, ServiceStatus::Running { .. }),
                    )
                    .await;
                    for (family, count) in [("other", 0), ("vendor", 1), ("other", 1)] {
                        let answer = fixture
                            .runtime
                            .operations
                            .call::<Inspect>(family.into())
                            .await
                            .unwrap();
                        if family == "vendor" {
                            assert_eq!(answer.as_str(), "egress status 200");
                        } else {
                            assert!(answer.contains("may not invoke operation"), "{answer}");
                            assert!(answer.contains("host.egress"), "{answer}");
                        }
                        assert_eq!(fixture.prepared.load(Ordering::SeqCst), count);
                        assert_eq!(fixture.calls.load(Ordering::SeqCst), count);
                    }
                    tokio::time::timeout(Duration::from_secs(5), fixture.runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    for marker in ["prepared", "initialized", "shutdown", "cleaned"] {
                        assert_eq!(
                            std::fs::read(fixture.root.path().join(marker)).unwrap(),
                            b"1\n"
                        );
                    }
                });
        });
    }
    {
        scenario_runtime::check("missing_provider_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let fixture = Fixture::new(false).await;
                    service_status(
                        fixture.runtime.services.subscribe(),
                        "native-core",
                        |status| matches!(status, ServiceStatus::Inactive { .. }),
                    )
                    .await;
                    assert!(!fixture.root.path().join("prepared").exists());
                    assert!(!fixture.root.path().join("initialized").exists());
                    assert_eq!(fixture.prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
                    assert!(matches!(
                        fixture.runtime.stop().await,
                        Err(harmos::Error::TaskFailed {
                            domain: "service",
                            ..
                        })
                    ));
                });
        });
    }
}

/* === Tests === */
