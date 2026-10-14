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
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{Notify, mpsc};
// Local Imports
use super::{Bank, assembly, service_status, support};
use harmos::{services::Fault, services::Service, services::ServiceExit, services::ServiceStatus};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("resident_not_running", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let entered = Arc::new(Notify::new());
                    let runtime = assembly()
                        .service("initializing", {
                            let entered = entered.clone();
                            move |mut context: Service<Bank>| {
                                let entered = entered.clone();
                                async move {
                                    entered.notify_one();
                                    context.stopping().await;
                                    context.ready(); // A late signal during cleanup is ignored.
                                    Ok::<(), Fault<&str>>(())
                                }
                            }
                        })
                        .runtime()
                        .await
                        .unwrap();
                    entered.notified().await;
                    assert_eq!(
                        runtime
                            .operations
                            .resident(&harmos::operations::resident::Key::root("initializing"))
                            .and_then(|item| item.health),
                        Some(ServiceStatus::Starting)
                    );
                    let services = runtime.operations.clone();
                    runtime.stop().await.unwrap();
                    assert_eq!(
                        services
                            .resident(&harmos::operations::resident::Key::root("initializing"))
                            .and_then(|item| item.health),
                        Some(ServiceStatus::Stopped)
                    );
                });
        });
    }
    {
        scenario_runtime::check("readiness_follows_initialization", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let entered = Arc::new(Notify::new());
                    let initialized = Arc::new(Notify::new());
                    let runtime = assembly()
                        .service("gated", {
                            let entered = entered.clone();
                            let initialized = initialized.clone();
                            move |mut context: Service<Bank>| {
                                let entered = entered.clone();
                                let initialized = initialized.clone();
                                async move {
                                    entered.notify_one();
                                    if context
                                        .until_stopping(initialized.notified())
                                        .await
                                        .is_some()
                                    {
                                        context.ready();
                                        context.ready();
                                        context.stopping().await;
                                    }
                                    Ok::<(), Fault<&str>>(())
                                }
                            }
                        })
                        .runtime()
                        .await
                        .unwrap();
                    entered.notified().await;
                    assert_eq!(
                        runtime
                            .operations
                            .resident(&harmos::operations::resident::Key::root("gated"))
                            .and_then(|item| item.health),
                        Some(ServiceStatus::Starting)
                    );
                    initialized.notify_one();
                    let mut statuses = runtime.operations.watch_residents();
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        statuses.wait_for(|values| {
                            matches!(
                                values
                                    .get(&harmos::operations::resident::Key::root("gated"))
                                    .and_then(|item| item.health.as_ref()),
                                Some(ServiceStatus::Running { .. })
                            )
                        }),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("initialization_preserves_credit", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let entered = Arc::new(Notify::new());
    let fail = Arc::new(Notify::new());
    let attempts = Arc::new(AtomicUsize::new(0));
    let runtime = assembly()
        .service_with(
            "unready",
            harmos::operations::OperationPolicy { restart: harmos::operations::RestartPolicy::OnTransient { credit: 1, sustained_health: Duration::from_secs(1), backoff: harmos::operations::Backoff::Exponential { initial: Duration::from_millis(10), maximum: Duration::from_secs(5), jitter: harmos::operations::Jitter::Deterministic } }, ..harmos::services::POLICY },
            {
                let entered = entered.clone();
                let fail = fail.clone();
                let attempts = attempts.clone();
                move |_context: Service<Bank>| {
                    let entered = entered.clone();
                    let fail = fail.clone();
                    let index = attempts.fetch_add(1, SeqCst);
                    async move {
                        if index > 0 {
                            entered.notify_one();
                            fail.notified().await;
                        }
                        Err::<(), _>(Fault::Environmental("not initialized"))
                    }
                }
            },
        )
        .runtime()
        .await
        .unwrap();
    entered.notified().await;
    tokio::time::advance(Duration::from_secs(10)).await;
    assert_eq!(
        runtime
            .operations
            .resident(&harmos::operations::resident::Key::root("unready"))
            .and_then(|item| item.health),
        Some(ServiceStatus::Starting)
    );
    fail.notify_one();
    let mut statuses = runtime.operations.watch_residents();
    tokio::time::timeout(
        Duration::from_secs(5),
        statuses.wait_for(|values| {
            matches!(
                values
                    .get(&harmos::operations::resident::Key::root("unready"))
                    .and_then(|item| item.health.as_ref()),
                Some(ServiceStatus::Inactive { .. })
            )
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        matches!(runtime.operations.resident(&harmos::operations::resident::Key::root("unready")).and_then(|item| item.health), Some(ServiceStatus::Inactive {
        cause: ServiceExit::CreditExhausted, history
    }) if history == ["not initialized", "not initialized"])
    );
    assert_eq!(attempts.load(SeqCst), 2);
    assert!(matches!(
        runtime.stop().await,
        Err(harmos::Error::TaskFailed {
            domain: "service",
            ..
        })
    ));

});
        });
    }
    {
        scenario_runtime::check("retirement_fences_readiness", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (retired, mut contexts) = mpsc::unbounded_channel();
                    let entered = Arc::new(Notify::new());
                    let initialized = Arc::new(Notify::new());
                    let attempts = Arc::new(AtomicUsize::new(0));
                    let runtime = assembly()
                        .service("replacement", {
                            let entered = entered.clone();
                            let initialized = initialized.clone();
                            move |mut context: Service<Bank>| {
                                let retired = retired.clone();
                                let entered = entered.clone();
                                let initialized = initialized.clone();
                                let index = attempts.fetch_add(1, SeqCst);
                                async move {
                                    if index == 0 {
                                        assert!(retired.send(context).is_ok());
                                        return Err(Fault::Environmental("replace"));
                                    }
                                    entered.notify_one();
                                    if context
                                        .until_stopping(initialized.notified())
                                        .await
                                        .is_some()
                                    {
                                        context.ready();
                                        context.stopping().await;
                                    }
                                    Ok(())
                                }
                            }
                        })
                        .runtime()
                        .await
                        .unwrap();
                    let old = contexts.recv().await.unwrap();
                    entered.notified().await;
                    old.ready();
                    tokio::task::yield_now().await;
                    assert_eq!(
                        runtime
                            .operations
                            .resident(&harmos::operations::resident::Key::root("replacement"))
                            .and_then(|item| item.health),
                        Some(ServiceStatus::Starting)
                    );
                    initialized.notify_one();
                    let mut statuses = runtime.operations.watch_residents();
                    tokio::time::timeout(
                        Duration::from_secs(5),
                        statuses.wait_for(|values| {
                            matches!(
                                values
                                    .get(&harmos::operations::resident::Key::root("replacement"))
                                    .and_then(|item| item.health.as_ref()),
                                Some(ServiceStatus::Running { .. })
                            )
                        }),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    drop(old);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("initialization_keeps_terminal", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    for panics in [false, true] {
                        let runtime = assembly()
                            .service("failed", move |_context: Service<Bank>| async move {
                                assert!(!panics, "initialization panic");
                                Err::<(), _>(Fault::Application("initialization refused"))
                            })
                            .runtime()
                            .await
                            .unwrap();
                        let mut statuses = runtime.operations.watch_residents();
                        tokio::time::timeout(
                            Duration::from_secs(5),
                            statuses.wait_for(|values| {
                                matches!(
                                    values
                                        .get(&harmos::operations::resident::Key::root("failed"))
                                        .and_then(|item| item.health.as_ref()),
                                    Some(ServiceStatus::Inactive { .. })
                                )
                            }),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                        let Some(ServiceStatus::Inactive { cause, history }) = runtime
                            .operations
                            .resident(&harmos::operations::resident::Key::root("failed"))
                            .and_then(|item| item.health)
                        else {
                            panic!("failed initialization is terminal");
                        };
                        assert_eq!(
                            cause,
                            if panics {
                                ServiceExit::Panicked
                            } else {
                                ServiceExit::Application("initialization refused".into())
                            }
                        );
                        assert!(history.is_empty());
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
    {
        scenario_runtime::check("activated_service_starting", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .runtime()
                        .await
                        .expect("a runtime that declares no service of its own boots");
                    let statuses = runtime.operations.watch_residents();
                    assert!(
                        statuses.borrow().is_empty(),
                        "nothing is on the watch before a tenant is admitted"
                    );

                    runtime
                        .operations
                        .register_service(
                            harmos::services::ServiceDefinition {
                                id: "tenant".into(),
                                policy: harmos::services::POLICY,
                            },
                            |_: Arc<()>| {
                                |context: harmos::services::Service<Bank>| async move {
                                    let mut context = context;
                                    context.ready();
                                    context.stopping().await;
                                    Ok::<(), Fault<String>>(())
                                }
                            },
                        )
                        .expect("the tenant factory is registered");
                    let _tenant = runtime
                        .operations
                        .submit_resident("tenant")
                        .await
                        .expect("a tenant is admitted after boot");

                    // Read on the next line, with nothing awaited in between. The supervisor
                    // writes its own first status from a task, and on this runtime a task
                    // cannot have run yet — so what is on the watch here is what `activate`
                    // itself put there, and a caller reading the name it just activated finds
                    // a service rather than a hole.
                    assert_eq!(
                        statuses
                            .borrow()
                            .get(&harmos::operations::resident::Key::root("tenant"))
                            .and_then(|item| item.health.as_ref()),
                        Some(&ServiceStatus::Starting),
                        "an activated tenant reads `Starting` before `activate` answers"
                    );

                    // The supervisor replaces that status rather than introducing the name, so
                    // there is no moment on the way to running when the tenant is missing.
                    service_status(statuses.clone(), "tenant", |status| {
                        matches!(status, ServiceStatus::Running { .. })
                    })
                    .await;
                    assert!(
                        statuses
                            .borrow()
                            .contains_key(&harmos::operations::resident::Key::root("tenant"))
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("context_publishes_streams", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .register(support::streams())
                        .service(
                            "source",
                            |context: harmos::services::Service<Bank>| async move {
                                context.streams().publish(support::pulse(7));
                                Ok::<(), Fault<&'static str>>(())
                            },
                        )
                        .runtime()
                        .await
                        .expect("the stream-producing service boots");
                    service_status(runtime.operations.watch_residents(), "source", |status| {
                        *status == ServiceStatus::Stopped
                    })
                    .await;
                    let rows: Vec<_> = runtime
                        .streams
                        .query::<support::Pulse>(..)
                        .await
                        .expect("the service's row remains in the window")
                        .collect();
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].value.sample, 7);
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */
