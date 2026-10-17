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
use harmos_runtime::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos_runtime::{Error, Runtime, services::Fault, services::Service, services::ServiceStatus};
use support::{Bank, Ledger, catalog};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Ping;

/* === Enums === */
#[harmos_runtime::operation(
        request = (),
        response = usize,
        id = "service.ping",
        policy = OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct PingContract;

#[harmos_runtime::catalog]
enum Operations {
    PingContract(PingContract),
}

#[harmos_runtime::lanes]
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

    async fn initialize<'ctx>(
        _resources: Arc<()>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        _request: &mut (),
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<usize>, Self::Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move { Ok(42) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn service_status(
    mut services: tokio::sync::watch::Receiver<
        BTreeMap<
            harmos_runtime::operations::resident::Key,
            harmos_runtime::operations::resident::Snapshot,
        >,
    >,
    name: &str,
    wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(status) = services
                .borrow()
                .get(&harmos_runtime::operations::resident::Key::root(name))
                .and_then(|item| item.health.clone())
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

fn assembly() -> harmos_runtime::Builder<Bank> {
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
    let manager = runtime.handle().operations;
    manager
        .register_service(
            harmos_runtime::services::ServiceDefinition {
                id: "dynamic".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |_: Arc<()>| resident,
        )
        .unwrap();
    let dynamic = manager
        .submit_resident("dynamic")
        .await
        .unwrap();
    assert_eq!(runtime.operations.residents().len(), 2);
    assert!(
        matches!(manager.register_service(harmos_runtime::services::ServiceDefinition { id: "boot".into(), policy: harmos_runtime::services::POLICY }, |_: Arc<()>| resident), Err(Error::AlreadyRegistered { domain: "service", name }) if name == "boot")
    );
    let boot = manager
        .submit_resident("boot")
        .await
        .unwrap();
    assert_eq!(manager.resident(&manager.resident_key(boot.id()).unwrap()).unwrap().leases, 2);
    boot.release().await.unwrap();
    assert_eq!(
        manager
            .resident(&harmos_runtime::operations::resident::Key::root("boot"))
            .unwrap()
            .leases,
        1
    );
    assert_eq!(manager.resident(&manager.resident_key(dynamic.id()).unwrap()).unwrap().leases, 1);
    runtime.stop().await.unwrap();
    assert_eq!(
        manager
            .resident(&harmos_runtime::operations::resident::Key::root("boot"))
            .and_then(|item| item.health),
        Some(ServiceStatus::Stopped)
    );
    assert_eq!(
        manager
            .resident(&harmos_runtime::operations::resident::Key::root("dynamic"))
            .and_then(|item| item.health),
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
                    let manager = runtime.operations.clone();
                    manager.shutdown_residents().await.unwrap();
                    assert!(matches!(
                        manager.submit_resident("late").await,
                        Err(Error::Closing { .. })
                    ));
                    runtime
                        .operations
                        .register_operation::<Ping>("service", Lanes::Open, Arc::new(()))
                        .unwrap();
                    assert_eq!(*runtime.operations.call(()).await.unwrap(), 42);
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
                            runtime.operations.register_service(
                                harmos_runtime::services::ServiceDefinition {
                                    id: name.into(),
                                    policy: harmos_runtime::services::POLICY
                                },
                                |_: Arc<()>| resident
                            ),
                            Err(Error::InvalidInput {
                                field: "service name",
                                ..
                            })
                        ));
                    }
                    assert!(runtime.operations.residents().is_empty());
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
                        let manager = runtime.operations.clone();
                        let barrier = barrier.clone();
                        let entered = entered.clone();
                        let exited = exited.clone();
                        producers.push(tokio::spawn(async move {
                            barrier.wait().await;
                            let id = format!("resident-{index}");
                            manager.register_service(
                                harmos_runtime::services::ServiceDefinition {
                                    id: id.clone(),
                                    policy: harmos_runtime::services::POLICY,
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
                            manager.submit_resident(&id).await
                        }));
                    }
                    barrier.wait().await;
                    runtime.operations.shutdown_residents().await.unwrap();
                    for producer in producers {
                        match producer.await.unwrap() {
                            Ok(_lease) => {}
                            Err(Error::Closing { .. }) => {}
                            Err(error) => panic!("unexpected admission: {error:?}"),
                        }
                    }
                    assert_eq!(
                        entered.load(Ordering::SeqCst),
                        exited.load(Ordering::SeqCst)
                    );
                    assert!(
                        runtime
                            .operations
                            .residents()
                            .values()
                            .all(|instance| instance.health == Some(ServiceStatus::Stopped))
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
                        let manager = runtime.operations.clone();
                        async move { manager.shutdown_residents().await }
                    });
                    stopping.notified().await;
                    let second = tokio::spawn({
                        let manager = runtime.operations.clone();
                        async move { manager.shutdown_residents().await }
                    });
                    tokio::task::yield_now().await;
                    assert!(!first.is_finished());
                    assert!(!second.is_finished());
                    release.notify_one();
                    first.await.unwrap().unwrap();
                    second.await.unwrap().unwrap();
                    assert_eq!(
                        runtime
                            .operations
                            .resident(&harmos_runtime::operations::resident::Key::root("boot"))
                            .and_then(|item| item.health),
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
