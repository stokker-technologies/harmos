/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[path = "../support/app/minimal.rs"]
mod app;
mod lifecycle;
mod policy;
mod readiness;
mod recovery;
mod scopes;
mod shutdown;

#[path = "../support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{Barrier, Notify};
// Local Imports
use harmos::operations::{Context, Operation, OperationPolicy, RetryPolicy};
use harmos::{
    Error, Runtime, services::Fault, services::Service, services::ServicePolicy,
    services::ServiceStatus,
};
use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Ping;

/* === Enums === */
#[harmos::operation(
        request = (),
        response = usize,
        id = "service.ping",
        policy = OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
        },
    )]
struct PingContract;

#[harmos::catalog]
enum Operations {
    PingContract(PingContract),
}

#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
    #[lane(serial)]
    Serial,
}

/* === Traits === */

/* === Implementations === */
impl Operation for Ping {
    type Declaration = PingContract;
    type Resources = ();
    type Error = std::convert::Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(_resources: Arc<()>) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<usize>, Self::Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move { Ok(42) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn service_status(
    mut services: tokio::sync::watch::Receiver<
        BTreeMap<harmos::services::ServiceKey, harmos::services::ServiceSnapshot>,
    >,
    name: &str,
    wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(status) = services
                .borrow()
                .get(&harmos::services::ServiceKey::root(name))
                .map(|item| item.status.clone())
                && wanted(&status)
            {
                return status;
            }
            services
                .changed()
                .await
                .expect("the runtime keeps the status sender open");
        }
    })
    .await
    .expect("the service reaches its expected status")
}

fn assembly() -> harmos::Builder<Bank> {
    Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .register(Lanes::catalog())
}

async fn resident(mut context: Service<Bank>) -> Result<(), Fault<&'static str>> {
    context.ready();
    context.stopping().await;
    Ok(())
}

fn own_contract() {
    {
        scenario_runtime::check("handle_clones_share", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let runtime = assembly()
        .service("boot", resident)
        .runtime()
        .await
        .unwrap();
    let manager = runtime.handle().services;
    manager
        .register(
            harmos::services::ServiceDefinition {
                id: "dynamic".into(),
                policy: ServicePolicy::default(),
            },
            |_: Arc<()>| resident,
        )
        .unwrap();
    let dynamic = manager
        .activate("dynamic", &manager.root_scope())
        .await
        .unwrap();
    assert_eq!(runtime.services.statuses().len(), 2);
    assert!(
        matches!(manager.register(harmos::services::ServiceDefinition { id: "boot".into(), policy: ServicePolicy::default() }, |_: Arc<()>| resident), Err(Error::AlreadyRegistered { domain: "service", name }) if name == "boot")
    );
    let boot = manager
        .activate("boot", &manager.root_scope())
        .await
        .unwrap();
    assert_eq!(manager.status(boot.key()).unwrap().leases, 2);
    boot.release().await.unwrap();
    assert_eq!(
        manager
            .status(&harmos::services::ServiceKey::root("boot"))
            .unwrap()
            .leases,
        1
    );
    assert_eq!(manager.status(dynamic.key()).unwrap().leases, 1);
    runtime.stop().await.unwrap();
    assert_eq!(
        manager
            .status(&harmos::services::ServiceKey::root("boot"))
            .map(|item| item.status),
        Some(ServiceStatus::Stopped)
    );
    assert_eq!(
        manager
            .status(&harmos::services::ServiceKey::root("dynamic"))
            .map(|item| item.status),
        Some(ServiceStatus::Stopped)
    );

});
        });
    }
    {
        scenario_runtime::check("shutdown_preserves_work", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly()
                        .service("boot", resident)
                        .runtime()
                        .await
                        .unwrap();
                    let manager = runtime.services.clone();
                    manager.shutdown().await.unwrap();
                    assert!(matches!(
                        runtime
                            .services
                            .activate("late", &manager.root_scope())
                            .await,
                        Err(Error::Closing { domain: "service" })
                    ));
                    runtime
                        .operations
                        .register_operation::<Ping>("service", Lanes::Open, Arc::new(()))
                        .unwrap();
                    assert_eq!(
                        *runtime.operations.call::<PingContract>(()).await.unwrap(),
                        42
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("admission_rejects_reserved", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    for name in ["", "artifact"] {
                        assert!(matches!(
                            runtime.services.register(
                                harmos::services::ServiceDefinition {
                                    id: name.into(),
                                    policy: ServicePolicy::default()
                                },
                                |_: Arc<()>| resident
                            ),
                            Err(Error::InvalidInput {
                                field: "service name",
                                ..
                            })
                        ));
                    }
                    assert!(runtime.services.statuses().is_empty());
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdown_join_started", || {
            let mut scenario_executor = tokio::runtime::Builder::new_multi_thread();
            scenario_executor.worker_threads(2);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = assembly().runtime().await.unwrap();
                    let barrier = Arc::new(Barrier::new(9));
                    let entered = Arc::new(AtomicUsize::new(0));
                    let exited = Arc::new(AtomicUsize::new(0));
                    let mut producers = Vec::new();
                    for index in 0..8 {
                        let manager = runtime.services.clone();
                        let barrier = barrier.clone();
                        let entered = entered.clone();
                        let exited = exited.clone();
                        producers.push(tokio::spawn(async move {
                            barrier.wait().await;
                            let id = format!("resident-{index}");
                            manager.register(
                                harmos::services::ServiceDefinition {
                                    id: id.clone(),
                                    policy: ServicePolicy::default(),
                                },
                                move |_: Arc<()>| {
                                    let entered = entered.clone();
                                    let exited = exited.clone();
                                    move |mut context: Service<Bank>| {
                                        let entered = entered.clone();
                                        let exited = exited.clone();
                                        async move {
                                            entered.fetch_add(1, Ordering::SeqCst);
                                            context.stopping().await;
                                            exited.fetch_add(1, Ordering::SeqCst);
                                            Ok::<(), Fault<&str>>(())
                                        }
                                    }
                                },
                            )?;
                            manager.activate(&id, &manager.root_scope()).await
                        }));
                    }
                    barrier.wait().await;
                    runtime.services.shutdown().await.unwrap();
                    for producer in producers {
                        match producer.await.unwrap() {
                            Ok(_lease) => {}
                            Err(Error::Closing { domain: "service" }) => {}
                            Err(error) => panic!("unexpected admission: {error:?}"),
                        }
                    }
                    assert_eq!(
                        entered.load(Ordering::SeqCst),
                        exited.load(Ordering::SeqCst)
                    );
                    assert!(
                        runtime
                            .services
                            .statuses()
                            .values()
                            .all(|instance| instance.status == ServiceStatus::Stopped)
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("shutdowns_wait_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let entered = Arc::new(Notify::new());
                    let stopping = Arc::new(Notify::new());
                    let release = Arc::new(Notify::new());
                    let runtime = assembly()
                        .service("boot", {
                            let entered = entered.clone();
                            let stopping = stopping.clone();
                            let release = release.clone();
                            move |mut context: Service<Bank>| {
                                let entered = entered.clone();
                                let stopping = stopping.clone();
                                let release = release.clone();
                                async move {
                                    entered.notify_one();
                                    context.stopping().await;
                                    stopping.notify_one();
                                    release.notified().await;
                                    Ok::<(), Fault<&str>>(())
                                }
                            }
                        })
                        .runtime()
                        .await
                        .unwrap();
                    entered.notified().await;
                    let first = tokio::spawn({
                        let manager = runtime.services.clone();
                        async move { manager.shutdown().await }
                    });
                    stopping.notified().await;
                    let second = tokio::spawn({
                        let manager = runtime.services.clone();
                        async move { manager.shutdown().await }
                    });
                    tokio::task::yield_now().await;
                    assert!(!first.is_finished());
                    assert!(!second.is_finished());
                    release.notify_one();
                    first.await.unwrap().unwrap();
                    second.await.unwrap().unwrap();
                    assert_eq!(
                        runtime
                            .services
                            .status(&harmos::services::ServiceKey::root("boot"))
                            .map(|item| item.status),
                        Some(ServiceStatus::Stopped)
                    );
                    runtime.stop().await.unwrap();
                });
        });
    }
}

/* === Tests === */

#[test]
fn service_readiness() {
    own_contract();
    lifecycle::contract_scenario();
    readiness::contract_scenario();
    recovery::contract_scenario();
}

#[test]
fn service_shutdown() {
    scopes::contract_scenario();
    shutdown::contract_scenario();
}
