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

#[path = "../support/process_core_contract.rs"]
mod contract;
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
struct Fixture {
    runtime: Runtime<Bank>,
    _lease: harmos::services::ServiceLease<Bank>,
    root: tempfile::TempDir,
    prepared: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

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
