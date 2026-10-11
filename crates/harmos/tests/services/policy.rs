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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{assembly, service_status, support::Bank};
use harmos::{
    FactSource, Fault, Position, Resident, Runtime, ServiceExit, ServicePolicy, ServiceStatus,
    SystemFact,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct MutableAdapter {
    attempts: usize,
    exits: tokio::sync::mpsc::UnboundedReceiver<AdapterExit>,
}

/* === Enums === */
enum AdapterExit {
    Environmental,
    Application,
}

/* === Traits === */

/* === Implementations === */
impl Resident<Bank> for MutableAdapter {
    type Error = String;

    async fn attempt(
        &mut self,
        mut context: harmos::Service<Bank>,
    ) -> Result<(), Fault<Self::Error>> {
        self.attempts += 1;
        let attempt = self.attempts;
        context.ready();
        match context.until_stopping(self.exits.recv()).await {
            None => Ok(()),
            Some(Some(AdapterExit::Environmental)) => {
                Err(Fault::Environmental(format!("reset on attempt {attempt}")))
            }
            Some(Some(AdapterExit::Application)) => {
                Err(Fault::Application(format!("failed on attempt {attempt}")))
            }
            Some(None) => Err(Fault::Application("exit channel closed".to_owned())),
        }
    }
    async fn finalize(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn work_facts(runtime: &Runtime<Bank>) -> Vec<SystemFact> {
    runtime
        .state
        .query::<SystemFact>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("service evidence is readable"))
        .map(|entry| entry.value)
        .filter(|fact| matches!(fact.source, FactSource::Work(_)))
        .collect()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("failure_panic_deactivate", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let application = assembly()
                        .service("analysis", |_context| async {
                            Err::<(), _>(Fault::Application("bad invariant"))
                        })
                        .runtime()
                        .await
                        .expect("the service declaration is valid");
                    let status =
                        service_status(application.services.subscribe(), "analysis", |status| {
                            matches!(status, ServiceStatus::Inactive { .. })
                        })
                        .await;
                    assert!(matches!(
                        status,
                        ServiceStatus::Inactive {
                            cause: ServiceExit::Application(ref error),
                            ref history,
                        } if error == "bad invariant" && history.is_empty()
                    ));
                    assert_eq!(work_facts(&application).await.len(), 1);
                    assert!(matches!(
                        application.stop().await,
                        Err(harmos::Error::TaskFailed {
                            domain: "service",
                            ..
                        })
                    ));

                    let panicked = assembly()
                        .service("capture", |_context| async {
                            panic!("capture invariant failed");
                            #[allow(unreachable_code)]
                            Ok::<(), Fault<&'static str>>(())
                        })
                        .runtime()
                        .await
                        .expect("the service declaration is valid");
                    let status =
                        service_status(panicked.services.subscribe(), "capture", |status| {
                            matches!(status, ServiceStatus::Inactive { .. })
                        })
                        .await;
                    assert!(matches!(
                        status,
                        ServiceStatus::Inactive {
                            cause: ServiceExit::Panicked,
                            ..
                        }
                    ));
                    assert_eq!(work_facts(&panicked).await.len(), 1);
                    assert!(matches!(
                        panicked.stop().await,
                        Err(harmos::Error::TaskFailed {
                            domain: "service",
                            ..
                        })
                    ));
                });
        });
    }
    {
        scenario_runtime::check("exhaustion_retains_attempt", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let attempts = Arc::new(AtomicUsize::new(0));
                    let service_attempts = Arc::clone(&attempts);
                    let runtime = assembly()
                        .service("adapter", move |_context| {
                            let attempts = Arc::clone(&service_attempts);
                            async move {
                                attempts.fetch_add(1, Ordering::SeqCst);
                                Err::<(), _>(Fault::Environmental("still disconnected"))
                            }
                        })
                        .runtime()
                        .await
                        .expect("the service declaration is valid");
                    let status =
                        service_status(runtime.services.subscribe(), "adapter", |status| {
                            matches!(status, ServiceStatus::Inactive { .. })
                        })
                        .await;

                    assert!(matches!(
                        status,
                        ServiceStatus::Inactive {
                            cause: ServiceExit::CreditExhausted,
                            ref history,
                        } if history == &["still disconnected"; 4]
                    ));
                    assert_eq!(attempts.load(Ordering::SeqCst), 4);
                    assert_eq!(work_facts(&runtime).await.len(), 1);
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
        scenario_runtime::check("credit_survive_restarts", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let (exits, receiver) = tokio::sync::mpsc::unbounded_channel();
                    let policy = ServicePolicy {
                        credit: 1,
                        sustained_health: Duration::from_millis(20),
                        backoff_base: Duration::from_millis(1),
                        backoff_cap: Duration::from_millis(1),
                        ..ServicePolicy::default()
                    };
                    let runtime = assembly()
                        .service_with(
                            "adapter",
                            policy,
                            MutableAdapter {
                                attempts: 0,
                                exits: receiver,
                            },
                        )
                        .runtime()
                        .await
                        .expect("the mutable resident declaration is valid");
                    let mut statuses = runtime.services.subscribe();
                    service_status(statuses.clone(), "adapter", |status| {
                        *status == ServiceStatus::Running { credit: 1 }
                    })
                    .await;

                    let key = harmos::services::ServiceKey::root("adapter");
                    let instance_id = runtime.services.status(&key).unwrap().id;

                    exits
                        .send(AdapterExit::Environmental)
                        .expect("the first attempt is live");
                    service_status(statuses.clone(), "adapter", |status| {
                        *status == ServiceStatus::Running { credit: 0 }
                    })
                    .await;
                    assert_eq!(runtime.services.status(&key).unwrap().id, instance_id);
                    statuses
                        .wait_for(|all| {
                            all.get(&harmos::services::ServiceKey::root("adapter"))
                                .map(|item| &item.status)
                                == Some(&ServiceStatus::Running { credit: 1 })
                        })
                        .await
                        .expect("sustained health refills the visible credit");

                    exits
                        .send(AdapterExit::Application)
                        .expect("the second attempt is live");
                    let status = service_status(statuses, "adapter", |status| {
                        matches!(status, ServiceStatus::Inactive { .. })
                    })
                    .await;
                    assert!(matches!(
                        status,
                        ServiceStatus::Inactive {
                            cause: ServiceExit::Application(ref error),
                            ref history,
                        } if error == "failed on attempt 2" && history == &["reset on attempt 1"]
                    ));
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
        scenario_runtime::check("service_deactivation_records", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .service("analysis", |_context| async {
                            Err::<(), _>(Fault::Application("bad state"))
                        })
                        .runtime()
                        .await
                        .expect("the service declaration is valid");
                    service_status(runtime.services.subscribe(), "analysis", |status| {
                        matches!(status, ServiceStatus::Inactive { .. })
                    })
                    .await;
                    let facts = work_facts(&runtime).await;
                    assert_eq!(facts.len(), 1);
                    assert_eq!(
                        facts[0].identify().0.as_str(),
                        "harmos:work/analysis/deactivated"
                    );
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
}
