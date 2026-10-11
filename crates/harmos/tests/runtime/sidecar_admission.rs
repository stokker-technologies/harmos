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

use crate::support::fixtures as prepared_fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::{fs, path::Path};
// Third Party Imports

// Local Imports
use super::*;
use harmos::sidecar::{Registrar, Registration};

/* === Constants === */
const NAME: &str = "admission-fixture";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn expected() -> Vec<Registration> {
    let mut registrar = Registrar::new();
    registrar.register("fixture", 1, &"first").unwrap();
    registrar.register("unknown", 2, &vec![3, 4]).unwrap();
    let mut entries = registrar.finish();
    entries.reverse();
    entries
}

fn notices(_: &Service<Bank>, notice: Notice) {
    if let Notice::Logged { line } = notice {
        assert_ne!(line, "panic at host", "the host notice handler panicked");
    }
}

async fn native(
    root: &Path,
    expected: &[Registration],
    policy: ServicePolicy,
) -> operations::Managed<Executable> {
    let source = Executable::at(prepared_fixtures::artifact("harmos-admission-fixture"))
        .unwrap()
        .variable("HARMOS_TEST_ADMISSION", root);
    let definitions = operations::inspected(&source, serde_json::to_vec(&()).unwrap());
    let resident = Supervised::new(source, notices as fn(&Service<Bank>, Notice))
        .expected_registrations(expected)
        .handshake(Duration::from_secs(2))
        .grace(Duration::from_millis(25));
    let runtime = assembly()
        .register(operations::Lanes::catalog())
        .operation_preparation_bytes(NonZeroUsize::new(4096).unwrap())
        .runtime()
        .await
        .unwrap();
    operations::Managed::from_runtime(
        runtime,
        definitions,
        resident,
        operations::Lanes::NativeOpen,
        NonZeroUsize::new(1).unwrap(),
        policy,
    )
}

fn setup() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("catalog"), "first").unwrap();
    root
}

async fn refuses_frozen_catalog(entries: Vec<Registration>) {
    let root = setup();
    let mut fixture = native(root.path(), &entries, ServicePolicy::default()).await;
    let ready = fixture.resident().ready();
    let registrations = fixture.resident().registrations();
    assert!(fixture.initialize().await.is_err());
    let runtime = &fixture.runtime;
    let status = service_status(runtime.services.subscribe(), NAME, |value| {
        matches!(value, ServiceStatus::Inactive { .. })
    })
    .await;
    assert!(
        matches!(status, ServiceStatus::Inactive { cause: ServiceExit::Application(ref reason), .. } if reason.contains("expected catalog")),
        "{status:?}"
    );
    assert!(root.path().join("registered").exists());
    assert!(!root.path().join("initialized").exists());
    assert!(!*ready.borrow());
    assert!(registrations.borrow().is_none());
    assert!(matches!(
        runtime.stop().await,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));
}

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("payload_drift_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut entries = expected();
                    entries[0].payload = b"[]".to_vec();
                    refuses_frozen_catalog(entries).await;
                });
        });
    }
    {
        scenario_runtime::check("schema_drift_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut entries = expected();
                    entries[0].schema += 1;
                    refuses_frozen_catalog(entries).await;
                });
        });
    }
    {
        scenario_runtime::check("missing_entry_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut entries = expected();
                    entries.pop();
                    refuses_frozen_catalog(entries).await;
                });
        });
    }
    {
        scenario_runtime::check("duplicate_entry_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let mut entries = expected();
                    entries.push(entries[0].clone());
                    refuses_frozen_catalog(entries).await;
                });
        });
    }
    {
        scenario_runtime::check("readiness_requires_init", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let root = setup();
    let mut fixture = native(root.path(), &expected(), ServicePolicy::default()).await;
    let calls = fixture.resident().clone();
    let ready = calls.ready();
    let mut registrations = calls.registrations();
    let services = fixture.runtime.services.clone();
    assert!(!*ready.borrow());
    {
        let initialization = fixture.initialize();
        tokio::pin!(initialization);
        tokio::select! {
            result = &mut initialization => panic!("SDK initialization passed the unreleased gate: {result:?}"),
            result = tokio::time::timeout(Duration::from_secs(3), registrations.wait_for(Option::is_some)) => {
                result.unwrap().unwrap();
            }
        }
        assert!(!*ready.borrow());
        assert_eq!(
            services
                .status(&harmos::services::ServiceKey::root(NAME))
                .map(|item| item.status),
            Some(ServiceStatus::Starting)
        );
        assert!(matches!(calls.operations(), Err(SidecarError::Unavailable)));
        fs::write(root.path().join("release"), "").unwrap();
        initialization.await.unwrap();
    }
    publications::ready(ready.clone()).await;
    service_status(services.subscribe(), NAME, |value| {
        matches!(value, ServiceStatus::Running { .. })
    })
    .await;
    assert_eq!(
        fs::read_to_string(root.path().join("initialized")).unwrap(),
        "called\n"
    );
    fixture.shutdown().await.unwrap();
    fixture.runtime.stop().await.unwrap();
    assert!(!*ready.borrow());
    assert!(registrations.borrow().is_none());

});
        });
    }
    {
        scenario_runtime::check("refused_init_unready", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let root = setup();
                    fs::write(root.path().join("refuse"), "").unwrap();
                    let mut fixture =
                        native(root.path(), &expected(), ServicePolicy::default()).await;
                    let ready = fixture.resident().ready();
                    let failure = fixture
                        .initialize()
                        .await
                        .expect_err("the actual SDK initialization refuses");
                    let mut cause: &dyn std::error::Error = &failure;
                    loop {
                        if cause.to_string().contains("initialization refused") {
                            break;
                        }
                        cause = cause
                            .source()
                            .expect("the original authored refusal remains in the cause chain");
                    }
                    assert!(root.path().join("initialized").exists());
                    assert!(!*ready.borrow());
                    assert_eq!(
                        fixture.status(NAME).unwrap().state,
                        harmos::load::PluginState::Failed
                    );
                    fixture.shutdown().await.unwrap();
                    fixture.runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("restart_closes_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for panic in [false, true] {
                        let root = setup();
                        fs::write(root.path().join("release"), "").unwrap();
                        let policy = ServicePolicy {
                            credit: 1,
                            sustained_health: Duration::from_secs(30),
                            backoff_base: Duration::from_millis(1),
                            backoff_cap: Duration::from_millis(2),
                            ..ServicePolicy::default()
                        };
                        let source =
                            Executable::at(prepared_fixtures::artifact("harmos-admission-fixture"))
                                .unwrap()
                                .variable("HARMOS_TEST_ADMISSION", root.path());
                        let mut definition = process_definition(&source);
                        definition.configuration = serde_json::to_vec(&()).unwrap();
                        let calls = Supervised::new(source, notices as fn(&Service<Bank>, Notice))
                            .expected_registrations(&expected())
                            .handshake(Duration::from_secs(2))
                            .grace(Duration::from_millis(25));
                        let ready = calls.ready();
                        let registrations = calls.registrations();
                        let runtime = assembly()
                            .operation_preparation_bytes(NonZeroUsize::new(4096).unwrap())
                            .runtime()
                            .await
                            .unwrap();
                        let resident = calls.clone();
                        runtime
                            .services
                            .register(
                                harmos::services::ServiceDefinition {
                                    id: NAME.into(),
                                    policy,
                                },
                                move |_: Arc<()>| {
                                    harmos::sidecar::Managed::new_executable(
                                        resident.clone(),
                                        definition.clone(),
                                        HostLanes::Open,
                                        NonZeroUsize::new(1).unwrap(),
                                        NonZeroUsize::new(4096).unwrap(),
                                    )
                                    .unwrap()
                                },
                            )
                            .unwrap();
                        let _lease = runtime
                            .services
                            .activate(NAME, &runtime.services.root_scope())
                            .await
                            .unwrap();
                        service_status(runtime.services.subscribe(), NAME, |status| {
                            matches!(status, ServiceStatus::Running { .. })
                        })
                        .await;
                        fs::write(root.path().join("catalog"), "changed").unwrap();
                        fs::write(root.path().join(if panic { "panic" } else { "crash" }), "")
                            .unwrap();
                        service_status(runtime.services.subscribe(), NAME, |value| {
                            matches!(value, ServiceStatus::Inactive { .. })
                        })
                        .await;
                        assert!(!*ready.borrow());
                        assert!(registrations.borrow().is_none());
                        assert!(matches!(calls.operations(), Err(SidecarError::Unavailable)));
                        assert_eq!(
                            fs::read_to_string(root.path().join("initialized")).unwrap(),
                            "called\n"
                        );
                        if !panic {
                            assert_eq!(
                                fs::read_to_string(root.path().join("registered")).unwrap(),
                                "called\ncalled\n"
                            );
                        }
                        assert!(matches!(
                            runtime.stop().await,
                            Err(harmos::Error::TaskFailed {
                                domain: "service",
                                ..
                            })
                        ));
                    }
                });
        });
    }
}

/* === Tests === */
