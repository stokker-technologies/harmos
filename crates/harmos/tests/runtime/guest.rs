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

#[path = "guest_core.rs"]
mod core;
#[path = "../../../harmos-guest/examples/route_component/contract.rs"]
mod wire;

#[path = "guest_operations.rs"]
mod operations;
#[path = "guest_publications.rs"]
mod publications;

/* === Re-Exports === */
use wire::{Decline, Echo, Echoed, Fail, Spin, Wait};

/* === Imports === */
// Standard Library Imports
use std::sync::OnceLock;
use std::time::Duration;
// Third Party Imports
use support::Bank;
// Local Imports
use crate::support;
use crate::support::fixtures::component as load;
use harmos::Message;
use harmos::Runtime;
use harmos::operations::Host as _;
use harmos::operations::{OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos::plugin::guest::{Error, Host};
use harmos_sidecar::Attach;

/* === Constants === */
const NATIVE_CAPACITY: usize = 64;
static NEXT_NATIVE_SEED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NATIVE_SEEDS: std::sync::Mutex<
    std::collections::BTreeMap<u64, std::sync::Weak<ProviderOwner>>,
> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos::message]
#[derive(Clone, PartialEq)]
struct Reserved {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos::operations::WireOperation for Forward {
    const VERSION: u32 = 1;
}

impl harmos::operations::WireOperation for ProviderEcho {
    const VERSION: u32 = 1;
}

impl OperationDeclaration for Reserved {
    type Request = Self;
    const ID: &'static str = "harmos.register";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(2),
        attempt_timeout: Duration::from_secs(2),
        retries: RetryPolicy::Never,
    };

    type Response = Echoed;
}

/* === Public Functions === */

/* === Private Functions === */
#[harmos::catalog]
enum GuestOperations {
    Echo(Echo),
    Decline(Decline),
    Spin(Spin),
    Wait(Wait),
    Fail(Fail),
}

#[harmos::lanes]
enum GuestLanes {
    #[default]
    Open,
}

async fn retire_failed_guest<L: harmos::plugin::PluginLifecycle>(
    manager: &mut harmos::plugin::PluginManager<L::Guest, L::Sidecar, L::Embedded>,
    lifecycle: &mut L,
) {
    let harmos::Error::PluginLifecycle { source } = manager.shutdown(lifecycle).await.unwrap_err()
    else {
        panic!("the original failed generation must report its SDK shutdown refusal")
    };
    let harmos::plugin::PluginFailure::Cleanup(failures) = *source else {
        panic!("the failed generation retains its original physical cleanup owner")
    };
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].artifact, "route-guest");
    let harmos::Error::TaskFailed { domain, source } = &failures[0].error else {
        panic!("the manager preserves its original cleanup failure");
    };
    assert_eq!(*domain, "plugin cleanup");
    let original = source
        .downcast_ref::<harmos::Error>()
        .expect("the original SDK error remains available");
    assert!(matches!(
        component_error(original),
        harmos::operations::ComponentError::Unavailable
    ));
    manager.shutdown(lifecycle).await.unwrap();
}

fn component_error(error: &harmos::Error) -> &harmos::operations::ComponentError {
    let harmos::Error::OperationFailed { source, .. } = error else {
        panic!("expected the original handler failure, got {error:?}");
    };
    source
        .downcast_ref()
        .expect("component handler source is retained")
}

fn execution_error(error: &harmos::Error) -> &wasmtime::Error {
    let harmos::operations::ComponentError::Transport(Error::Execution { source }) =
        component_error(error)
    else {
        panic!("expected the original guest execution failure, got {error:?}");
    };
    source
}

#[test]
fn contract_scenario() {
    scenario_runtime::check("host_resource_settings", || {
        let mut executor = tokio::runtime::Builder::new_current_thread();
        executor.enable_all().build().unwrap().block_on(async {
            let runtime = runtime().await;
            let bytes = route_component();
            let settings = harmos::config::PluginGuestSettings {
                component_bytes: std::num::NonZeroUsize::new(bytes.len() - 1).unwrap(),
                ..Default::default()
            };
            let restricted = Host::with_settings(&runtime, settings).unwrap();
            assert!(matches!(restricted.load(&bytes), Err(Error::Oversized { limit }) if limit == bytes.len() - 1));
            let settings = harmos::config::PluginGuestSettings {
                memory_bytes: std::num::NonZeroUsize::new(1).unwrap(),
                ..Default::default()
            };
            let restricted = Host::with_settings(&runtime, settings).unwrap();
            let refused = restricted.load(&bytes).unwrap().register().await;
            let error = match refused { Ok(_) => panic!("one byte cannot admit the component's initial memory"), Err(error) => error };
            assert!(error.to_string().contains("memory"), "{error}");
            let allowed = Host::new(&runtime).unwrap().load(&bytes).unwrap().register().await.unwrap();
            drop(allowed);
            drop(restricted);
            runtime.stop().await.unwrap();
        });
    });
    {
        scenario_runtime::check("memory_growth_bounded", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = publications::Lifecycle::new(&runtime);
                    let mut manager =
                        publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let refused = runtime
                        .operations
                        .call_as::<Echo>(Echo { say: "grow".into() })
                        .await
                        .unwrap_err();
                    assert!(
                        execution_error(&refused)
                            .root_cause()
                            .to_string()
                            .starts_with("forcing trap when growing memory to "),
                        "{refused}"
                    );
                    let after = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "after".into(),
                        })
                        .await
                        .unwrap_err();
                    assert!(matches!(
                        component_error(&after),
                        harmos::operations::ComponentError::Unavailable
                    ));
                    retire_failed_guest(&mut manager, &mut lifecycle).await;
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("inert_plan_denies", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use harmos::plugin::PluginDefinitions;
                    use std::sync::Arc;
                    use std::sync::atomic::{AtomicUsize, Ordering};

                    let runtime = runtime().await;
                    let calls = Arc::new(AtomicUsize::new(0));
                    let mut lifecycle = publications::Lifecycle::with_egress(
                        &runtime,
                        Duration::ZERO,
                        calls.clone(),
                        Arc::new(AtomicUsize::new(0)),
                    );
                    let definitions = lifecycle.candidate("idle").await;
                    let invalid = definitions.definitions().cloned().map(|mut definition| {
                        if definition.id == "route-guest" {
                            definition.configuration = b"{}".to_vec();
                        }
                        definition
                    });
                    let mut manager =
                        publications::Manager::new(PluginDefinitions::new(invalid, []).unwrap())
                            .unwrap();
                    assert!(manager.caller("route-guest").is_err());
                    assert!(matches!(
                        runtime
                            .operations
                            .call_as::<Echo>(Echo {
                                say: "egress:vendor".into(),
                            })
                            .await,
                        Err(harmos::Error::NotRegistered {
                            domain: "operation",
                            ..
                        })
                    ));
                    assert_eq!(calls.load(Ordering::SeqCst), 0);
                    assert!(manager.initialize(&mut lifecycle).await.is_err());
                    assert_eq!(
                        calls.load(Ordering::SeqCst),
                        0,
                        "invalid configuration cannot reach the Core handler"
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();

                    let plan = manager
                        .plan_reload(lifecycle.candidate("idle").await)
                        .unwrap();
                    assert!(manager.caller("route-guest").is_err());
                    assert_eq!(
                        calls.load(Ordering::SeqCst),
                        0,
                        "collection and planning retain an inert original store"
                    );
                    manager.apply_reload(plan, &mut lifecycle).await.unwrap();
                    assert!(manager.caller("route-guest").is_ok());
                    // Verify Activation With Ordinary Egress, Not The Five-Millisecond Deadline Probe.
                    let response = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "egress:vendor".into(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(response.said, "egress status 200");
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("egress_grant_checked", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::Arc;
                    use std::sync::atomic::{AtomicUsize, Ordering};

                    let runtime = runtime().await;
                    let calls = Arc::new(AtomicUsize::new(0));
                    let prepared = Arc::new(AtomicUsize::new(0));
                    let mut lifecycle = publications::Lifecycle::with_egress(
                        &runtime,
                        Duration::ZERO,
                        calls.clone(),
                        prepared.clone(),
                    );
                    let mut manager =
                        publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let refused = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "egress:other".into(),
                        })
                        .await
                        .unwrap();
                    assert!(
                        refused.said.contains("may not invoke operation"),
                        "{}",
                        refused.said
                    );
                    assert!(refused.said.contains("host.egress"), "{}", refused.said);
                    assert_eq!(prepared.load(Ordering::SeqCst), 0);
                    assert_eq!(calls.load(Ordering::SeqCst), 0);
                    let accepted = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "egress:vendor".into(),
                        })
                        .await
                        .unwrap();
                    assert_eq!(accepted.said, "egress status 200");
                    assert_eq!(prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    let refused = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "egress:other".into(),
                        })
                        .await
                        .unwrap();
                    assert!(
                        refused.said.contains("may not invoke operation"),
                        "{}",
                        refused.said
                    );
                    assert_eq!(prepared.load(Ordering::SeqCst), 1);
                    assert_eq!(calls.load(Ordering::SeqCst), 1);
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("declared_operation_roundtrip", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = runtime().await;
    let mut lifecycle = publications::Lifecycle::new(&runtime);
    let mut manager = publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let answered = runtime
        .operations
        .call_as::<Echo>(Echo {
            say: "the probe".into(),
        })
        .await
        .unwrap();
    assert_eq!(answered.said, "heard the probe");
    let denied = runtime
        .operations
        .call_as::<Echo>(Echo {
            say: "publish-foreign".into(),
        })
        .await
        .unwrap();
    assert!(
        denied.said.contains("publication stream"),
        "{}",
        denied.said
    );
    assert!(
        denied
            .said
            .contains("stream or schema differs from the frozen generation inventory"),
        "{}",
        denied.said
    );
    assert!(runtime.streams.publication("foreign", 1).is_none());

    let operation = ["probe", "echo"].join(".");
    let encoded = runtime
        .operations
        .call_encoded(
            &operation,
            Echo {
                say: "dynamic".into(),
            }
            .encode_to_vec(),
        )
        .await
        .unwrap();
    assert_eq!(
        Echoed::decode(encoded.as_slice()).unwrap().said,
        "heard dynamic"
    );
    assert!(matches!(
        runtime
            .operations
            .call_encoded("probe.unknown", vec![])
            .await,
        Err(harmos::Error::NotRegistered {
            domain: "operation",
            ..
        })
    ));

    let refused = runtime
        .operations
        .call_as::<Decline>(Decline {})
        .await
        .unwrap_err();
    assert!(
        matches!(component_error(&refused), harmos::operations::ComponentError::Remote {
        failure: harmos::operations::Failure::Permanent, message,
    } if message.contains("the probe declines"))
    );
    assert!(harmos::operations::Definition::of::<Reserved>().is_err());
    assert_eq!(
        runtime
            .operations
            .call_as::<Echo>(Echo {
                say: "again".into()
            })
            .await
            .unwrap()
            .said,
        "heard again"
    );
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("fuel_retains_cause", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = publications::Lifecycle::new(&runtime);
                    let mut manager =
                        publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let spun = runtime
                        .operations
                        .call_as::<Spin>(Spin {})
                        .await
                        .unwrap_err();
                    assert_eq!(
                        execution_error(&spun).downcast_ref::<wasmtime::Trap>(),
                        Some(&wasmtime::Trap::OutOfFuel)
                    );
                    let after = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "anyone".into(),
                        })
                        .await
                        .unwrap_err();
                    assert!(matches!(
                        component_error(&after),
                        harmos::operations::ComponentError::Unavailable
                    ));
                    retire_failed_guest(&mut manager, &mut lifecycle).await;
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("manager_bounds_wait", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = publications::Lifecycle::with_egress(
                        &runtime,
                        Duration::from_millis(200),
                        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                        std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                    );
                    let mut manager =
                        publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let waited = runtime
                        .operations
                        .call_as::<Wait>(Wait {})
                        .await
                        .unwrap_err();
                    assert!(
                        matches!(
                            waited,
                            harmos::Error::OperationRefused(harmos::Refusal::TimedOut)
                        ),
                        "{waited:?}"
                    );
                    let after = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "anyone".into(),
                        })
                        .await
                        .unwrap_err();
                    assert!(matches!(
                        component_error(&after),
                        harmos::operations::ComponentError::Unavailable
                    ));
                    retire_failed_guest(&mut manager, &mut lifecycle).await;
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("trap_discards_instance", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let mut lifecycle = publications::Lifecycle::new(&runtime);
                    let mut manager =
                        publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let trapped = runtime
                        .operations
                        .call_as::<Fail>(Fail {})
                        .await
                        .unwrap_err();
                    assert_ne!(
                        execution_error(&trapped).downcast_ref::<wasmtime::Trap>(),
                        Some(&wasmtime::Trap::OutOfFuel)
                    );
                    assert!(
                        execution_error(&trapped)
                            .downcast_ref::<wasmtime::Trap>()
                            .is_some()
                    );
                    let after = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "anyone".into(),
                        })
                        .await
                        .unwrap_err();
                    assert!(matches!(
                        component_error(&after),
                        harmos::operations::ComponentError::Unavailable
                    ));
                    retire_failed_guest(&mut manager, &mut lifecycle).await;
                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("retired_metadata_refused", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let host = Host::<Bank>::new(&runtime).unwrap();
                    let error = match host.load(&legacy_component()) {
                        Ok(_) => {
                            panic!("a current world cannot rehabilitate retired artifact metadata")
                        }
                        Err(error) => error,
                    };
                    assert!(matches!(
                        error,
                        harmos::plugin::guest::Error::Declaration(
                            harmos::plugin::guest::inventory::DeclarationError::IncompatiblePayload
                        )
                    ));
                    assert!(runtime.operations.definitions().is_empty());
                    drop(host);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("scripted_ending_repeats", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    assert_eq!(scripted().await, scripted().await);
                });
        });
    }
    {
        scenario_runtime::check("catalog_resources_bounded", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for case in [
                        "valid",
                        "count",
                        "bytes",
                        "memory",
                        "spin",
                        "namespace",
                        "schema",
                        "duplicate",
                    ] {
                        let component = load("catalog-component", "componentize,kit", Some(case));
                        let result = tokio::time::timeout(
                            Duration::from_secs(30),
                            harmos::plugin::guest::registrations(&component),
                        )
                        .await
                        .expect("collection and runtime cleanup finish");
                        match case {
                            "valid" => {
                                let entries = result.expect("register-only collection succeeds");
                                assert_eq!(
                                    entries
                                        .iter()
                                        .map(|entry| entry.namespace.as_str())
                                        .collect::<Vec<_>>(),
                                    ["alpha.catalog", "zeta.catalog"]
                                );
                                assert_eq!(entries[0].schema, 1);
                                assert_eq!(entries[0].payload, br#""first""#);
                            }
                            "count" => {
                                assert!(result.unwrap_err().to_string().contains("256 namespaces"))
                            }
                            "namespace" | "schema" | "duplicate" => assert!(
                                result
                                    .unwrap_err()
                                    .to_string()
                                    .contains("invalid or duplicate")
                            ),
                            "spin" => {
                                let error = result.unwrap_err().to_string();
                                assert!(
                                    error.contains("fuel"),
                                    "loop refusal identifies its exhausted fuel: {error}"
                                );
                            }
                            "bytes" => {
                                let error = result.unwrap_err().to_string();
                                assert!(error.contains("16 MiB"), "catalog byte budget: {error}");
                            }
                            "memory" => {
                                let error = result.unwrap_err().to_string();
                                assert!(error.contains("memory"), "memory growth refusal: {error}");
                                assert!(
                                    !error.contains("fuel"),
                                    "memory limit must be reached first: {error}"
                                );
                            }
                            _ => unreachable!("all fixture cases are covered"),
                        }
                    }
                    assert!(
                        harmos::plugin::guest::registrations(b"not a component")
                            .await
                            .is_err()
                    );
                });
        });
    }
    {
        scenario_runtime::check("dependencies_share_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = runtime().await;
    let provider = std::sync::Arc::new(ProviderOwner::default());
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
    let mut manager = harmos::plugin::PluginManager::new(
        dependency_candidate(&mut lifecycle, "dependencies").await,
    )
    .unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    assert_eq!(
        runtime
            .operations
            .call_as::<Echo>(Echo {
                say: "dependency:normal".into()
            })
            .await
            .unwrap()
            .said,
        "normal"
    );
    for (say, message) in [
        ("refuse", "provider refusal"),
        ("timeout", "deadline elapsed"),
    ] {
        let error = runtime
            .operations
            .call_as::<Echo>(Echo {
                say: format!("dependency:{say}"),
            })
            .await
            .unwrap_err();
        assert!(
            matches!(component_error(&error), harmos::operations::ComponentError::Remote { failure: harmos::operations::Failure::Permanent, message: actual } if actual.contains(message)),
            "{error:?}"
        );
    }
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(*provider.observed.lock().unwrap(), ["initialize", "normal"]);

    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("cancelled_dependency_admission", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let provider = std::sync::Arc::new(ProviderOwner::default());
                    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
                    let mut manager = harmos::plugin::PluginManager::new(
                        dependency_candidate(&mut lifecycle, "idle").await,
                    )
                    .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let invocation = runtime
                        .operations
                        .submit_as::<Echo>(Echo {
                            say: "dependency:cancel".into(),
                        })
                        .unwrap();
                    provider.started.notified().await;
                    invocation.cancel();
                    assert!(matches!(
                        invocation.await,
                        Err(harmos::Error::OperationRefused(harmos::Refusal::Cancelled))
                    ));
                    tokio::time::timeout(Duration::from_secs(1), provider.cancelled.notified())
                        .await
                        .unwrap();
                    let after = runtime
                        .operations
                        .call_as::<Echo>(Echo {
                            say: "after cancellation".into(),
                        })
                        .await
                        .unwrap_err();
                    assert!(matches!(
                        component_error(&after),
                        harmos::operations::ComponentError::Unavailable
                    ));
                    retire_failed_guest(&mut manager, &mut lifecycle).await;

                    drop((manager, lifecycle));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("background_cleanup_retained", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use std::sync::{Arc, atomic::Ordering};
    let runtime = runtime().await;
    let provider = Arc::new(ProviderOwner::default());
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
    let mut manager =
        harmos::plugin::PluginManager::new(dependency_candidate(&mut lifecycle, "idle").await)
            .unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let prepared = provider.prepared.load(Ordering::SeqCst);

    // The Saved Initialization Context Uses The Original Generation And Provider.
    let answered = runtime
        .operations
        .call_as::<Echo>(Echo {
            say: "saved-dependency:original-background".into(),
        })
        .await
        .unwrap();
    assert_eq!(answered.said, "original-background");
    provider.fail_cleanup.store(true, Ordering::SeqCst);
    let refused = runtime
        .operations
        .call_as::<Echo>(Echo {
            say: "saved-dependency:physical-cleanup".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        component_error(&refused),
        harmos::operations::ComponentError::Remote { .. }
    ));
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 2);
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 0);

    // Destroying The Actual Guest Store Does Not Settle Its Failed Host Debt.
    let trapped = runtime.operations.call_as::<Fail>(Fail {}).await.unwrap_err();
    assert!(
        execution_error(&trapped)
            .downcast_ref::<wasmtime::Trap>()
            .is_some()
    );
    let first = manager.shutdown(&mut lifecycle).await.unwrap_err();
    let harmos::Error::PluginLifecycle { source } = first else {
        panic!("the failed guest must report its original shutdown refusal");
    };
    let harmos::plugin::PluginFailure::Cleanup(failures) = *source else {
        panic!("the failed guest must retain its original ending");
    };
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].artifact, "route-guest");

    let harmos::Error::TaskFailed { domain, source } = &failures[0].error else {
        panic!("the manager preserves its original cleanup failure");
    };
    assert_eq!(*domain, "plugin cleanup");
    let original = source
        .downcast_ref::<harmos::Error>()
        .expect("the original SDK error remains available");
    assert!(matches!(
        component_error(original),
        harmos::operations::ComponentError::Unavailable
    ));

    for _ in 0..2 {
        let harmos::Error::PluginLifecycle { source } =
            manager.shutdown(&mut lifecycle).await.unwrap_err()
        else {
            panic!("the original host cleanup must remain retryable");
        };
        let harmos::plugin::PluginFailure::Cleanup(failures) = *source else {
            panic!("the original cleanup failure must remain inspectable");
        };
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].artifact, "route-guest");
        let mut source = Some(&failures[0].error as &dyn std::error::Error);
        let mut original_refusal = false;
        while let Some(error) = source {
            original_refusal |= error
                .to_string()
                .contains("fixture physical cleanup refused");
            source = error.source();
        }
        assert!(original_refusal);
        assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 2);
        assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 0);
    }
    let failed_attempts = provider.cleanup_attempts.load(Ordering::SeqCst);
    assert!(failed_attempts >= 2);
    assert_eq!(
        provider.observed.lock().unwrap().as_slice(),
        ["original-background", "physical-cleanup"]
    );

    // A Cancelled Cleanup Wait Keeps The Same Physical Finalizer In Flight.
    provider.fail_cleanup.store(false, Ordering::SeqCst);
    provider.hold_cleanup.store(true, Ordering::SeqCst);
    let mut waiting = Box::pin(manager.shutdown(&mut lifecycle));
    tokio::select! {
        result = &mut waiting => panic!("original cleanup is still gated: {result:?}"),
        _ = provider.cleanup_started.notified() => {},
    }
    drop(waiting);
    assert_eq!(
        provider.cleanup_attempts.load(Ordering::SeqCst),
        failed_attempts + 1
    );
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 0);
    assert!(
        runtime
            .operations
            .call_as::<Echo>(Echo {
                say: "saved-dependency:after-retirement".into(),
            })
            .await
            .is_err()
    );
    provider.cleanup_release.notify_one();
    manager.shutdown(&mut lifecycle).await.unwrap();
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 2);
    assert_eq!(
        provider.cleanup_attempts.load(Ordering::SeqCst),
        failed_attempts + 1
    );
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 1);
    drop((answered, manager, lifecycle));
    runtime.stop().await.unwrap();

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
                    let runtime = runtime().await;
                    let provider = std::sync::Arc::new(ProviderOwner::default());
                    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
                    let candidate = dependency_candidate(&mut lifecycle, "idle").await;
                    let missing = candidate
                        .definitions()
                        .filter(|definition| definition.id != "provider")
                        .cloned();
                    // No Plugin Provides It, So Planning Routes It To The Host, Whose Preflight Refuses It
                    let planned = harmos::plugin::PluginDefinitions::new(missing, []).unwrap();
                    let routed = planned
                        .host_requirements()
                        .map(|(definition, required)| {
                            (
                                definition.id.clone(),
                                required.contract.operation().id.to_string(),
                            )
                        })
                        .filter(|(_, operation)| operation == "provider.echo")
                        .collect::<Vec<_>>();
                    assert_eq!(
                        routed,
                        [("route-guest".to_owned(), "provider.echo".to_owned())]
                    );
                    assert!(matches!(
                        runtime.operations.validate_requirements(&planned),
                        Err(harmos::Error::NotRegistered {
                            domain: "host provider",
                            ..
                        })
                    ));
                    assert!(provider.observed.lock().unwrap().is_empty());
                    let mut operations: Vec<_> = runtime
                        .operations
                        .definitions()
                        .iter()
                        .map(|operation| operation.id.to_string())
                        .collect();
                    operations.sort();
                    assert_eq!(operations, ["host.egress", "host.invoke", "host.publish"]);
                    drop(lifecycle);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_fences_callers", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::Arc;
                    let runtime = runtime().await;
                    let provider = Arc::new(ProviderOwner::default());
                    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
                    let mut manager = harmos::plugin::PluginManager::new(
                        native_dependency_candidate(&mut lifecycle, "dependencies", true).await,
                    )
                    .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let stale = runtime
                        .operations
                        .caller(&manager.caller("client").unwrap())
                        .unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Forward>(Echo {
                                say: "dependency:roundtrip".into()
                            })
                            .await
                            .unwrap()
                            .said,
                        "roundtrip"
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert_eq!(
                        *provider.observed.lock().unwrap(),
                        ["initialize", "roundtrip"]
                    );
                    assert!(
                        stale
                            .call_as::<Echo>(Echo {
                                say: "closed".into()
                            })
                            .await
                            .is_err()
                    );
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert!(
                        !provider
                            .observed
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|say| say == "cleanup")
                    );

                    drop((manager, lifecycle, stale));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("cancelled_shutdown_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    use std::sync::{Arc, atomic::Ordering};
    let runtime = runtime().await;
    let provider = Arc::new(ProviderOwner::default());
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
    let mut manager = harmos::plugin::PluginManager::new(
        native_dependency_candidate(&mut lifecycle, "dependencies", false).await,
    )
    .unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let prepared = provider.prepared.load(Ordering::SeqCst);
    provider.hold_cleanup.store(true, Ordering::SeqCst);
    let invocation = runtime
        .operations
        .submit_as::<ProviderEcho>(Echo {
            say: "physical-cleanup".into(),
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), provider.cleanup_started.notified())
        .await
        .unwrap();
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 1);
    let mut waiter = Box::pin(manager.shutdown(&mut lifecycle));
    tokio::select! {
        result = &mut waiter => panic!("shutdown returned before original physical cleanup settled: {result:?}"),
        _ = tokio::time::sleep(Duration::from_millis(20)) => {},
    }
    drop(waiter);
    assert!(!provider.hold_cleanup.load(Ordering::SeqCst));
    assert!(
        runtime
            .operations
            .call_as::<Echo>(Echo {
                say: "closed".into()
            })
            .await
            .is_err()
    );
    provider.cleanup_release.notify_one();
    manager.shutdown(&mut lifecycle).await.unwrap();
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(invocation.await.unwrap().said, "physical-cleanup");
    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 1);
    assert_eq!(provider.cleanup_attempts.load(Ordering::SeqCst), 1);
    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 1);

    drop((manager, lifecycle));
    runtime.stop().await.unwrap();

});
        });
    }
    {
        scenario_runtime::check("dependencies_revoke_instances", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use std::sync::{Arc, atomic::Ordering};
                    let runtime = runtime().await;
                    let provider = Arc::new(ProviderOwner::default());
                    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
                    let mut manager = harmos::plugin::PluginManager::new(
                        native_dependency_candidate(&mut lifecycle, "idle", false).await,
                    )
                    .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    let old = runtime
                        .operations
                        .caller(&manager.caller("route-guest").unwrap())
                        .unwrap();
                    let invocation = runtime
                        .operations
                        .submit_as::<Echo>(Echo {
                            say: "dependency:cancel".into(),
                        })
                        .unwrap();
                    tokio::time::timeout(Duration::from_secs(1), provider.started.notified())
                        .await
                        .unwrap();
                    invocation.cancel();
                    assert!(matches!(
                        invocation.await,
                        Err(harmos::Error::OperationRefused(harmos::Refusal::Cancelled))
                    ));
                    tokio::time::timeout(Duration::from_secs(1), provider.cancelled.notified())
                        .await
                        .unwrap();
                    retire_failed_guest(&mut manager, &mut lifecycle).await;
                    assert!(
                        old.call_as::<ProviderEcho>(Echo {
                            say: "stale".into()
                        })
                        .await
                        .is_err()
                    );

                    let mut manager = harmos::plugin::PluginManager::new(
                        native_dependency_candidate(&mut lifecycle, "dependencies", false).await,
                    )
                    .unwrap();
                    manager.initialize(&mut lifecycle).await.unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .call_as::<Echo>(Echo {
                                say: "replacement".into()
                            })
                            .await
                            .unwrap()
                            .said,
                        "heard replacement"
                    );
                    assert!(
                        old.call_as::<ProviderEcho>(Echo {
                            say: "stale replacement".into()
                        })
                        .await
                        .is_err()
                    );
                    let prepared = provider.prepared.load(Ordering::SeqCst);
                    provider.fail_cleanup.store(true, Ordering::SeqCst);
                    let mut debt = runtime
                        .operations
                        .submit_as::<ProviderEcho>(Echo {
                            say: "physical-cleanup".into(),
                        })
                        .unwrap();
                    assert!(matches!(
                        (&mut debt).await,
                        Err(harmos::Error::OperationFinalization { outcome: None, .. })
                    ));
                    for attempt in 0..2 {
                        let harmos::Error::PluginLifecycle { source } =
                            manager.shutdown(&mut lifecycle).await.unwrap_err()
                        else {
                            panic!(
                                "failed physical cleanup must remain owned by the original manager"
                            )
                        };
                        let harmos::plugin::PluginFailure::Cleanup(failures) = *source else {
                            panic!("original cleanup failure expected")
                        };
                        assert_eq!(failures.len(), 1);
                        assert_eq!(failures[0].artifact, "provider");
                        let harmos::Error::TaskFailed { domain, source } = &failures[0].error
                        else {
                            panic!("the manager retains the original provider cleanup error");
                        };
                        assert_eq!(*domain, "plugin cleanup");
                        let original = source.downcast_ref::<harmos::Error>().unwrap();
                        if attempt == 0 {
                            assert!(matches!(
                                original,
                                harmos::Error::OperationFinalization { .. }
                            ));
                        } else {
                            assert!(matches!(
                                original,
                                harmos::Error::OperationSettlement { .. }
                            ));
                        }
                        let mut source = Some(&failures[0].error as &dyn std::error::Error);
                        let mut physical_refusal = false;
                        while let Some(error) = source {
                            physical_refusal |= error
                                .to_string()
                                .contains("fixture physical cleanup refused");
                            source = error.source();
                        }
                        assert!(
                            physical_refusal,
                            "the same original physical refusal remains inspectable"
                        );
                    }
                    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 1);
                    assert_eq!(provider.cleanup_attempts.load(Ordering::SeqCst), 3);
                    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 0);
                    assert!(
                        runtime
                            .operations
                            .call_as::<Echo>(Echo {
                                say: "closed".into()
                            })
                            .await
                            .is_err()
                    );
                    provider.fail_cleanup.store(false, Ordering::SeqCst);
                    manager.shutdown(&mut lifecycle).await.unwrap();
                    assert_eq!(provider.prepared.load(Ordering::SeqCst), prepared + 1);
                    assert_eq!(provider.cleanup_attempts.load(Ordering::SeqCst), 4);
                    assert_eq!(provider.cleanup_finished.load(Ordering::SeqCst), 1);
                    drop(debt);
                    drop((manager, lifecycle, old));
                    runtime.stop().await.unwrap();
                });
        });
    }
}

async fn scripted() -> Vec<String> {
    let runtime = runtime().await;
    let mut lifecycle = publications::Lifecycle::new(&runtime);
    let mut manager = publications::Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let mut trace = vec![];
    for say in ["one", "two"] {
        trace.push(
            runtime
                .operations
                .call_as::<Echo>(Echo { say: say.into() })
                .await
                .unwrap()
                .said
                .clone(),
        );
    }
    let refused = runtime
        .operations
        .call_as::<Decline>(Decline {})
        .await
        .unwrap_err();
    let harmos::operations::ComponentError::Remote { failure, message } = component_error(&refused)
    else {
        panic!("expected classified refusal");
    };
    trace.push(format!("{failure:?}: {message}"));
    let trapped = runtime
        .operations
        .call_as::<Fail>(Fail {})
        .await
        .unwrap_err();
    trace.push(format!(
        "trapped: {:?}",
        execution_error(&trapped)
            .downcast_ref::<wasmtime::Trap>()
            .unwrap()
    ));
    let after = runtime
        .operations
        .call_as::<Echo>(Echo {
            say: "after".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        component_error(&after),
        harmos::operations::ComponentError::Unavailable
    ));
    trace.push("unavailable".into());
    retire_failed_guest(&mut manager, &mut lifecycle).await;
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
    trace
}

async fn runtime() -> Runtime<Bank> {
    Runtime::builder(support::Ledger::default(), ())
        .register(GuestLanes::catalog())
        .register(support::catalog())
        .operation_settings(harmos::operations::Settings {
            preparation_bytes: Some(std::num::NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .expect("an empty ledger has nothing to recover")
}

fn route_component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("route-component", "test-component", None))
        .clone()
}

fn legacy_component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("legacy-component", "componentize,kit", None))
        .clone()
}

struct ProviderEcho;
impl OperationDeclaration for ProviderEcho {
    type Request = Echo;
    type Response = Echoed;
    const ID: &'static str = "provider.echo";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_millis(100),
        attempt_timeout: Duration::from_millis(100),
        retries: RetryPolicy::Never,
    };
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Shared;
}
#[harmos::catalog]
enum ProviderOperations {
    Echo(ProviderEcho),
}

#[derive(Default)]
struct ProviderOwner {
    prepared: std::sync::atomic::AtomicUsize,
    cleanup_attempts: std::sync::atomic::AtomicUsize,
    cleanup_finished: std::sync::atomic::AtomicUsize,
    started: tokio::sync::Notify,
    cancelled: std::sync::Arc<tokio::sync::Notify>,
    observed: std::sync::Mutex<Vec<String>>,
    discard_observations: bool,
    hold_cleanup: std::sync::atomic::AtomicBool,
    fail_cleanup: std::sync::atomic::AtomicBool,
    cleanup_started: tokio::sync::Notify,
    cleanup_release: tokio::sync::Notify,
}
struct Cancelled(std::sync::Arc<tokio::sync::Notify>);
impl Drop for Cancelled {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}
#[harmos::catalog]
enum EmbeddedOperations {
    Read(EmbeddedReading),
}

struct EmbeddedReading {
    resources: std::sync::Arc<ProviderOwner>,
    cleanup: bool,
}

impl harmos::operations::Operation for EmbeddedReading {
    type Declaration = ProviderEcho;
    type Resources = ProviderOwner;
    type Error = harmos_contract::Refusal;
    type Endpoints = ();
    type Context<'a> = harmos::operations::Context<'a>;
    async fn initialize<'ctx>(
        resources: std::sync::Arc<ProviderOwner>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        resources
            .prepared
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self {
            resources,
            cleanup: false,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Echo,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Echoed>, Self::Error> {
        let result: ::core::result::Result<Echoed, Self::Error> = async move {
            self.cleanup = request.say == "physical-cleanup";
            self.resources.answer(request).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.cleanup(self.cleanup).await
    }
}
impl ProviderOwner {
    async fn answer(&self, state: &Echo) -> Result<Echoed, harmos_contract::Refusal> {
        if state.say == "cancel" {
            let _cancelled = Cancelled(self.cancelled.clone());
            self.started.notify_one();
            std::future::pending::<()>().await;
        }
        if state.say == "timeout" {
            std::future::pending::<()>().await;
        }
        if state.say == "refuse" {
            return Err(harmos_contract::Refusal::new(73, "provider refusal"));
        }
        if !self.discard_observations {
            self.observed.lock().unwrap().push(state.say.clone());
        }
        Ok(Echoed {
            said: state.say.clone(),
        })
    }
    async fn cleanup(&self, physical: bool) -> Result<(), harmos_contract::Refusal> {
        use std::sync::atomic::Ordering;
        if !physical {
            return Ok(());
        }
        self.cleanup_attempts.fetch_add(1, Ordering::SeqCst);
        if self.fail_cleanup.load(Ordering::SeqCst) {
            return Err(harmos_contract::Refusal::new(
                1,
                "fixture physical cleanup refused",
            ));
        }
        if self.hold_cleanup.swap(false, Ordering::SeqCst) {
            self.cleanup_started.notify_one();
            self.cleanup_release.notified().await;
        }
        self.cleanup_finished.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
struct Forward;
impl OperationDeclaration for Forward {
    type Request = Echo;
    type Response = Echoed;
    const ID: &'static str = "client.forward";
    const POLICY: OperationPolicy = Echo::POLICY;
    const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Shared;
}
#[harmos::catalog]
enum ForwardOperations {
    Forward(Forward),
}
struct NativeProvider(std::sync::Arc<ProviderOwner>);
struct NativeClient;
struct NativeReading {
    resources: std::sync::Arc<NativeProvider>,
    cleanup: bool,
}
impl harmos::operations::Operation for NativeReading {
    type Declaration = ProviderEcho;
    type Resources = NativeProvider;
    type Error = harmos_contract::Refusal;
    type Endpoints = ();
    type Context<'a> = &'a harmos_sidecar::Supervisor;
    async fn initialize<'ctx>(
        resources: std::sync::Arc<NativeProvider>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        resources
            .0
            .prepared
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Self {
            resources,
            cleanup: false,
        })
    }
    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Echo,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Echoed>, Self::Error> {
        let result: ::core::result::Result<Echoed, Self::Error> = async move {
            self.cleanup = request.say == "physical-cleanup";
            self.resources.0.answer(request).await
        }
        .await;
        result.map(::core::convert::Into::into)
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        self.resources.0.cleanup(self.cleanup).await
    }
}
struct NativeForwarding {
    _resources: std::sync::Arc<NativeClient>,
}
impl harmos::operations::Operation for NativeForwarding {
    type Declaration = Forward;
    type Resources = NativeClient;
    type Error = harmos_contract::Refusal;
    type Endpoints = ();
    type Context<'a> = &'a harmos_sidecar::Supervisor;
    async fn initialize<'ctx>(
        resources: std::sync::Arc<NativeClient>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            _resources: resources,
        })
    }
    async fn run<'ctx>(
        &mut self,
        host: Self::Context<'ctx>,
        request: &mut Echo,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<Echoed>, Self::Error> {
        let result: ::core::result::Result<Echoed, Self::Error> = async move {
            host.call(request.clone())
                .await
                .map(harmos::operations::Response::into_owned)
                .map_err(Self::Error::of)
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
struct NativeSeed {
    id: u64,
}
impl NativeSeed {
    fn new(provider: &std::sync::Arc<ProviderOwner>) -> std::io::Result<Self> {
        use std::sync::atomic::Ordering;
        let id = NEXT_NATIVE_SEED
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |id| id.checked_add(1))
            .map_err(|_| std::io::Error::other("native fixture resource identities exhausted"))?;
        let mut seeds = NATIVE_SEEDS.lock().unwrap();
        if seeds.len() >= NATIVE_CAPACITY {
            return Err(std::io::Error::other(
                "native fixture resource handoff is full",
            ));
        }
        seeds.insert(id, std::sync::Arc::downgrade(provider));
        Ok(Self { id })
    }
}
impl Drop for NativeSeed {
    fn drop(&mut self) {
        NATIVE_SEEDS.lock().unwrap().remove(&self.id);
    }
}
struct NativeProviderPlugin {
    resources: std::sync::Arc<NativeProvider>,
}
impl harmos::operations::lifecycle::Policy for NativeProviderPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}
#[harmos::plugin(id = "provider", provides = ProviderOperations, requires = ())]
impl harmos::Plugin for NativeProviderPlugin {
    type Context = harmos_sidecar::Supervisor;
    type Config = u64;
    type Error = std::io::Error;
    type Resources = NativeProvider;
    fn prepare(id: u64) -> Result<Self, Self::Error> {
        let provider = NATIVE_SEEDS
            .lock()
            .unwrap()
            .remove(&id)
            .and_then(|resource| resource.upgrade())
            .ok_or_else(|| {
                std::io::Error::other("the original native fixture resource is unavailable")
            })?;
        Ok(Self {
            resources: std::sync::Arc::new(NativeProvider(provider)),
        })
    }
    fn resources(&self) -> std::sync::Arc<NativeProvider> {
        self.resources.clone()
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn bind_operations(
        &self,
        table: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<NativeReading>(self.resources())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
struct NativeClientPlugin {
    resources: std::sync::Arc<NativeClient>,
}
#[harmos::catalog]
enum NativeRequirements {
    Echo(Echo),
}
impl harmos::operations::lifecycle::Policy for NativeClientPlugin {
    const INITIALIZATION_POLICY: OperationPolicy = Echo::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = Echo::POLICY;
}
#[harmos::plugin(id = "client", provides = ForwardOperations, requires = NativeRequirements)]
impl harmos::Plugin for NativeClientPlugin {
    type Context = harmos_sidecar::Supervisor;
    type Config = ();
    type Error = std::io::Error;
    type Resources = NativeClient;
    fn prepare(_: ()) -> Result<Self, Self::Error> {
        Ok(Self {
            resources: std::sync::Arc::new(NativeClient),
        })
    }
    fn resources(&self) -> std::sync::Arc<NativeClient> {
        self.resources.clone()
    }
    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    fn bind_operations(
        &self,
        table: &mut harmos::operations::WireTable<Self::Context>,
    ) -> Result<(), harmos::operations::ServingError> {
        table.bind_operation::<NativeForwarding>(self.resources())
    }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
#[derive(Clone)]
struct NativeAttachment {
    id: String,
    _seed: Option<std::sync::Arc<NativeSeed>>,
}
impl harmos_sidecar::Attach for NativeAttachment {
    type Error = std::io::Error;
    fn owner(&self) -> &str {
        &self.id
    }
    fn operations(&self) -> harmos_contract::Result<Vec<harmos::operations::Definition>> {
        if self.id == "provider" {
            native_provider().operations()
        } else {
            native_client().operations()
        }
    }
    fn operation_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos::operations::Contract>>> {
        if self.id == "provider" {
            native_provider().operation_contracts()
        } else {
            native_client().operation_contracts()
        }
    }
    fn required_contracts(
        &self,
    ) -> harmos_contract::Result<Option<Vec<harmos_contract::inventory::contracts::Required>>> {
        if self.id == "provider" {
            native_provider().required_contracts()
        } else {
            native_client().required_contracts()
        }
    }
    fn sdk_lifecycle(
        &self,
    ) -> harmos_contract::Result<Option<[harmos::operations::lifecycle::Descriptor; 2]>> {
        if self.id == "provider" {
            native_provider().sdk_lifecycle()
        } else {
            native_client().sdk_lifecycle()
        }
    }
    async fn attach(&self) -> Result<harmos_sidecar::Attached, Self::Error> {
        if self.id == "provider" {
            native_provider().attach().await
        } else {
            native_client().attach().await
        }
    }
}
fn native_provider() -> harmos_sidecar::PluginNative<NativeProviderPlugin> {
    harmos_sidecar::PluginNative::new(std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap())
        .unwrap()
}
fn native_client() -> harmos_sidecar::PluginNative<NativeClientPlugin> {
    harmos_sidecar::PluginNative::new(std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap())
        .unwrap()
}
fn dependency_notice(_: &harmos::services::Service<Bank>, _: harmos::plugin::sidecar::Notice) {}
type NativeResident = harmos::plugin::sidecar::Supervised<
    Bank,
    NativeAttachment,
    fn(&harmos::services::Service<Bank>, harmos::plugin::sidecar::Notice),
>;
#[derive(Default)]
struct NativeResources {
    services: Option<harmos::services::ServiceManager<Bank>>,
    sdk: Option<harmos::operations::PluginSdkRegistration>,
    initializing: Option<harmos::operations::EncodedInvocation>,
    shutting_down: Option<harmos::operations::EncodedInvocation>,
    initialized: bool,
    shutdown_attempted: bool,
    resident: Option<NativeResident>,
    lease: Option<harmos::services::ServiceLease<Bank>>,
    registration: Option<harmos::operations::PluginRegistration>,
    registered: bool,
}
type GuestResources = publications::Resources;
#[derive(Default)]
struct ProviderResources {
    endpoint: Option<harmos::plugin::PluginEndpoint>,
    registration: Option<harmos::operations::PluginRegistration>,
}
type DependencyBackends =
    harmos::plugin::PluginBackend<GuestResources, NativeResources, ProviderResources>;
struct DependencyHost {
    native_seed: Option<std::sync::Arc<NativeSeed>>,
    guest: publications::Lifecycle,
    operations: harmos::operations::OperationManager,
    provider: std::sync::Arc<ProviderOwner>,
    services: harmos::services::ServiceManager<Bank>,
}
impl DependencyHost {
    fn new(runtime: &Runtime<Bank>, provider: std::sync::Arc<ProviderOwner>) -> Self {
        Self {
            native_seed: None,
            guest: publications::Lifecycle::new(runtime),
            operations: runtime.operations.clone(),
            provider,
            services: runtime.services.clone(),
        }
    }
}
impl harmos::plugin::PluginLifecycle for DependencyHost {
    type Guest = GuestResources;
    type Sidecar = NativeResources;
    type Embedded = ProviderResources;
    fn preflight(&mut self, candidate: &harmos::plugin::PluginDefinitions) -> harmos::Result<()> {
        self.operations.validate_requirements(candidate)
    }
    fn prepare(
        &mut self,
        definition: &harmos::plugin::PluginDefinition,
    ) -> harmos::Result<DependencyBackends> {
        Ok(match definition.kind() {
            harmos::plugin::PluginKind::Guest => {
                harmos::plugin::PluginBackend::Guest(self.guest.guest_seed())
            }
            harmos::plugin::PluginKind::Embedded => {
                harmos::plugin::PluginBackend::Embedded(ProviderResources::default())
            }
            harmos::plugin::PluginKind::Sidecar => {
                harmos::plugin::PluginBackend::Sidecar(NativeResources {
                    services: Some(self.services.clone()),
                    ..NativeResources::default()
                })
            }
        })
    }
    async fn fence(
        &mut self,
        _: harmos::plugin::PluginTransition,
        _: &[harmos::plugin::PluginDefinition],
    ) -> harmos::Result<()> {
        Ok(())
    }
    async fn start(
        &mut self,
        definition: &harmos::plugin::PluginDefinition,
        router: &harmos::plugin::Router,
        authority: &harmos::plugin::PluginLifecycleAuthority<'_>,
        backend: &mut DependencyBackends,
    ) -> harmos::Result<()> {
        match backend {
            harmos::plugin::PluginBackend::Embedded(resources) => {
                resources.endpoint = Some(router.enter(&definition.id).unwrap());
                let scope = resources.endpoint.as_ref().unwrap().scope().unwrap();
                resources.registration = Some(self.operations.stage_plugin(scope, |bindings| {
                    bindings.register_wire_operations::<EmbeddedOperations, _>(
                        GuestLanes::Open,
                        self.provider.clone(),
                    )
                })?);
            }
            harmos::plugin::PluginBackend::Guest(resources) => {
                resources
                    .start::<GuestOperations, _>(
                        &self.operations,
                        definition,
                        router,
                        authority,
                        GuestLanes::Open,
                    )
                    .await?;
            }
            harmos::plugin::PluginBackend::Sidecar(resources) => {
                let resident: NativeResident = harmos::plugin::sidecar::Supervised::new(
                    NativeAttachment {
                        id: definition.id.clone(),
                        _seed: if definition.id == "provider" {
                            self.native_seed.take()
                        } else {
                            None
                        },
                    },
                    dependency_notice
                        as fn(&harmos::services::Service<Bank>, harmos::plugin::sidecar::Notice),
                )
                .routed(router);
                resources.resident = Some(resident.clone());
                let mut ready = resident.ready();
                self.services.register(
                    harmos::services::ServiceDefinition {
                        id: definition.id.clone(),
                        policy: harmos::services::ServicePolicy::default(),
                    },
                    move |_: std::sync::Arc<()>| resident.clone(),
                )?;
                resources.registered = true;
                resources.lease = Some(
                    self.services
                        .activate(&definition.id, &self.services.root_scope())
                        .await?,
                );
                let handle = if definition.contributions.lifecycle.is_some() {
                    let native = tokio::time::timeout(
                        Duration::from_secs(2),
                        resources.resident.as_ref().unwrap().staged_operations(),
                    )
                    .await
                    .map_err(|source| harmos::Error::TaskFailed {
                        domain: "fixture staged native endpoint",
                        source: std::sync::Arc::new(source),
                    })?
                    .map_err(|source| harmos::Error::OperationFailed {
                        operation: definition.id.clone(),
                        source: std::sync::Arc::new(source),
                    })?;
                    std::sync::Arc::new(native)
                } else {
                    tokio::time::timeout(Duration::from_secs(2), ready.wait_for(|ready| *ready))
                        .await
                        .unwrap()
                        .unwrap();
                    std::sync::Arc::new(resources.resident.as_ref().unwrap().operations().unwrap())
                };
                resources.registration = Some(self.operations.stage_plugin(
                    handle.scope().unwrap(),
                    |bindings| {
                        if definition.id == "provider" {
                            bindings.register_native_catalog::<ProviderOperations>(
                                GuestLanes::Open,
                                handle.clone(),
                                std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap(),
                            )
                        } else {
                            bindings.register_native_catalog::<ForwardOperations>(
                                GuestLanes::Open,
                                handle.clone(),
                                std::num::NonZeroUsize::new(NATIVE_CAPACITY).unwrap(),
                            )
                        }
                    },
                )?);
                if definition.contributions.lifecycle.is_some() {
                    resources.sdk = Some(handle.stage_sdk(&self.operations, GuestLanes::Open)?);
                    resources.initializing = Some(
                        resources.sdk.as_ref().unwrap().submit_initialize(
                            authority,
                            &definition.configuration,
                            std::num::NonZeroUsize::new(
                                2 * definition.configuration.len()
                                    + std::mem::size_of::<
                                        harmos::operations::lifecycle::InitializeRequest,
                                    >()
                                    + 64
                                    + 20,
                            )
                            .unwrap(),
                        )?,
                    );
                    let result = resources.initializing.as_mut().unwrap().await;
                    resources.initializing.take();
                    result?;
                    resources.initialized = true;
                }
            }
        }
        Ok(())
    }
    async fn stop(
        definition: &harmos::plugin::PluginDefinition,
        authority: &harmos::plugin::PluginLifecycleAuthority<'_>,

        backend: &mut DependencyBackends,
    ) -> harmos::Result<()> {
        match backend {
            harmos::plugin::PluginBackend::Guest(resources) => {
                resources.stop(authority).await?;
            }
            harmos::plugin::PluginBackend::Embedded(resources) => {
                if let Some(registration) = &mut resources.registration {
                    registration.shutdown().await?;
                }
                resources.registration.take();
                resources.endpoint.take();
            }
            harmos::plugin::PluginBackend::Sidecar(resources) => {
                if let Some(sdk) = &resources.sdk {
                    sdk.settle().await?;
                    if let Some(initializing) = &mut resources.initializing {
                        let result = initializing.await;
                        resources.initializing.take();
                        result?;
                        resources.initialized = true;
                    }
                    if resources.initialized && !resources.shutdown_attempted {
                        resources.shutting_down = Some(sdk.submit_shutdown(authority)?);
                        resources.shutdown_attempted = true;
                    }
                    if let Some(shutdown) = &mut resources.shutting_down {
                        let result = shutdown.await;
                        resources.shutting_down.take();
                        result?;
                        resources.initialized = false;
                    }
                }
                if let Some(registration) = &mut resources.registration {
                    registration.shutdown().await?;
                }
                if let Some(lease) = resources.lease.take() {
                    lease.release().await?;
                }
                if resources.registered {
                    resources
                        .services
                        .as_ref()
                        .unwrap()
                        .unregister(&definition.id)
                        .await?;
                }
                resources.registered = false;
                if let Some(sdk) = &mut resources.sdk {
                    sdk.close().await?;
                }
                resources.sdk.take();
                resources.registration.take();
                resources.resident.take();
            }
        }
        Ok(())
    }
}
async fn dependency_candidate(
    lifecycle: &mut DependencyHost,
    configuration: &str,
) -> harmos::plugin::PluginDefinitions {
    lifecycle.guest.candidate(configuration).await
}

async fn native_dependency_candidate(
    lifecycle: &mut DependencyHost,
    configuration: &str,
    client: bool,
) -> harmos::plugin::PluginDefinitions {
    use harmos::plugin::{
        PluginArtifact, PluginContributions, PluginDefinition, PluginDefinitions, Requirement,
    };
    let mut definitions: Vec<_> = dependency_candidate(lifecycle, configuration)
        .await
        .definitions()
        .cloned()
        .collect();
    assert!(lifecycle.native_seed.is_none());
    let seed = std::sync::Arc::new(NativeSeed::new(&lifecycle.provider).unwrap());
    let provider = definitions
        .iter_mut()
        .find(|definition| definition.id == "provider")
        .unwrap();
    provider.artifact = PluginArtifact::LocalSidecar {
        revision: "fixture".into(),
    };
    provider.configuration = serde_json::to_vec(&seed.id).unwrap();
    let carrier = native_provider();
    provider.contributions = PluginContributions {
        lifecycle: carrier.sdk_lifecycle().unwrap(),
        operations: carrier.operations().unwrap(),
        wire_contracts: carrier.operation_contracts().unwrap().unwrap(),
        required_contracts: carrier.required_contracts().unwrap(),
        ..Default::default()
    };
    lifecycle.native_seed = Some(seed);
    if client {
        definitions.push(PluginDefinition {
            id: "client".into(),
            version: "1.0.0".parse().unwrap(),
            artifact: PluginArtifact::LocalSidecar {
                revision: "fixture".into(),
            },
            configuration: b"null".to_vec(),
            contributions: PluginContributions {
                lifecycle: native_client().sdk_lifecycle().unwrap(),
                operations: native_client().operations().unwrap(),
                wire_contracts: native_client().operation_contracts().unwrap().unwrap(),
                required_contracts: native_client().required_contracts().unwrap(),
                ..Default::default()
            },
            requires: vec![Requirement {
                id: "route-guest".into(),
                version: "*".parse().unwrap(),
            }],
        });
    }
    PluginDefinitions::new(definitions, []).unwrap()
}

/* === Tests === */

#[tokio::test]
#[ignore = "communication benchmark; selected explicitly by mise bench"]
async fn native_guest_benchmark() {
    const SAMPLES: usize = 20;
    const ITERATIONS: usize = 50;
    let runtime = runtime().await;
    let provider = std::sync::Arc::new(ProviderOwner {
        discard_observations: true,
        ..ProviderOwner::default()
    });
    let mut lifecycle = DependencyHost::new(&runtime, provider.clone());
    let mut manager = harmos::plugin::PluginManager::new(
        native_dependency_candidate(&mut lifecycle, "dependencies", true).await,
    )
    .unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    for size in [64, 1024] {
        let payload = "x".repeat(size);
        let request = format!("dependency:{payload}");
        let round_trip = || async {
            let response = runtime
                .operations
                .call_as::<Forward>(Echo {
                    say: request.clone(),
                })
                .await
                .unwrap();
            assert_eq!(response.said, payload);
            std::hint::black_box(response);
        };
        for _ in 0..10 {
            round_trip().await;
        }
        let mut samples = Vec::with_capacity(SAMPLES);
        for _ in 0..SAMPLES {
            let start = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                round_trip().await;
            }
            samples.push(start.elapsed().as_nanos() as f64 / ITERATIONS as f64);
        }
        samples.sort_by(f64::total_cmp);
        let mean = samples.iter().sum::<f64>() / SAMPLES as f64;
        let measurement = serde_json::json!({
            "name": format!("native_guest_native/in_process/round_trip/{size}"),
            "mean": mean,
            "unit": "ns",
            "throughput": {"Bytes": 2 * size},
            "statistics": {
                "samples": samples,
                "iterations_per_sample": ITERATIONS,
                "warmup_iterations": 10,
                "setup_timed": false,
                "payload_bytes": size,
                "native_carrier": "in-process PluginNative",
                "guest_carrier": "Wasmtime component",
                "included": "request construction, managed admission, serialization, nested calls, response validation, physical finalization"
            }
        });
        println!("\nHARMOS_BENCHMARK {measurement}");
    }
    assert!(provider.observed.lock().unwrap().is_empty());
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[test]
fn guest_contracts() {
    operations::contract_scenario();
    publications::contract_scenario();
}
