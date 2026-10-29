/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[allow(clippy::duplicate_mod, reason = "the minimal app binds each domain's own `super::Lanes`")]
#[path = "../support/app/minimal.rs"]
mod app;
#[path = "../services/lifecycle.rs"]
mod lifecycle;
#[path = "../services/policy.rs"]
mod policy;
#[path = "../services/readiness.rs"]
mod readiness;
#[path = "../services/recovery.rs"]
mod recovery;
#[path = "../services/scopes.rs"]
mod scopes;
#[path = "../services/shutdown.rs"]
mod shutdown;

use crate::support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::{Barrier, Notify};

use harmos_runtime::operations::RetryPolicy;
use harmos_runtime::{Error, Runtime, services::ServiceContext, services::ServiceStatus};
use support::{Bank, Ledger, catalog};

#[harmos_runtime::operation(resources = [], response = u64,
        id = "service.ping",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(120),
            attempt_timeout: Duration::from_secs(120),
            retries: RetryPolicy::Never,
        },
    )]
struct PingContractDeclaration {}

#[harmos_runtime::catalog]
enum Operations {
    PingContractDeclaration(PingContractDeclaration),
}

use support::Lanes;

impl harmos_runtime::operations::Operation for PingContractDeclaration {
    type Error = std::convert::Infallible;

    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<u64>, Self::Error> {
        let result: ::core::result::Result<u64, Self::Error> = async move { Ok(42) }.await;
        result.map(::core::convert::Into::into)
    }
}

async fn service_status(
    mut services: tokio::sync::watch::Receiver<
        BTreeMap<harmos_runtime::operations::resident::Key, harmos_runtime::operations::resident::Snapshot>,
    >,
    name: &str, wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    services
        .wait_for(|all| {
            all.get(&harmos_runtime::operations::resident::Key::root(name)).and_then(|item| item.health.as_ref()).is_some_and(&wanted)
        })
        .await
        .expect("the runtime keeps the status sender open")
        .get(&harmos_runtime::operations::resident::Key::root(name))
        .and_then(|item| item.health.clone())
        .expect("the matched status is present")
}

fn assembly(setup: &mut harmos_runtime::Setup<Bank>) {
    setup.settings().register(catalog());
}

async fn resident(mut context: ServiceContext<Bank>) -> Result<(), std::io::Error> {
    context.ready();
    context.stopping().await;
    Ok(())
}

#[tokio::test]
async fn handle_shares_manager() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("boot", resident);
    })
    .await
    .unwrap();
    let manager = runtime.handle().operations;
    manager
        .register_service(
            harmos_runtime::services::ServiceDefinition {
                id: "dynamic".into(),
                policy: harmos_runtime::services::POLICY,
            },
            |_: Arc<harmos_sdk::ExecutionContext>| resident,
        )
        .unwrap();
    let dynamic = manager.submit_resident("dynamic").await.unwrap();
    assert_eq!(runtime.operations.residents().len(), 2);
    assert_eq!(runtime.operations.resident_key(dynamic.id()), Some(harmos_runtime::operations::resident::Key::root("dynamic")));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn duplicate_service_refused() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("boot", resident);
    })
    .await
    .unwrap();
    assert!(matches!(
        runtime.operations.register_service(
            harmos_runtime::services::ServiceDefinition {
                id: "boot".into(),
                policy: harmos_runtime::services::POLICY
            },
            |_: Arc<harmos_sdk::ExecutionContext>| resident
        ),
        Err(Error::AlreadyRegistered { domain: "service", name }) if name == "boot"
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_preserves_work() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("boot", resident);
    })
    .await
    .unwrap();
    let manager = runtime.operations.clone();
    manager.shutdown_residents().await.unwrap();
    assert!(matches!(manager.submit_resident("late").await, Err(Error::Closing { .. })));
    runtime.operations.register::<Operations>(Lanes::Open).await.unwrap();
    assert_eq!(*runtime.operations.call::<PingContractDeclaration>(PingContractDeclaration {}).await.unwrap(), 42);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn admission_rejects_reserved() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
    assert!(matches!(
        runtime.operations.register_service(
            harmos_runtime::services::ServiceDefinition {
                id: String::new(),
                policy: harmos_runtime::services::POLICY
            },
            |_: Arc<harmos_sdk::ExecutionContext>| resident
        ),
        Err(Error::InvalidInput { field: "service name", .. })
    ));
    assert!(runtime.operations.residents().is_empty());
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_join_started() {
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
    })
    .await
    .unwrap();
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
                move |_: Arc<harmos_sdk::ExecutionContext>| {
                    let entered = entered.clone();
                    let exited = exited.clone();
                    move |mut context: ServiceContext<Bank>| {
                        let entered = entered.clone();
                        let exited = exited.clone();
                        async move {
                            entered.fetch_add(1, Ordering::SeqCst);
                            context.stopping().await;
                            exited.fetch_add(1, Ordering::SeqCst);
                            Ok::<(), std::io::Error>(())
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
    assert_eq!(entered.load(Ordering::SeqCst), exited.load(Ordering::SeqCst));
    assert!(runtime.operations.residents().values().all(|instance| instance.health == Some(ServiceStatus::Stopped)));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_shutdowns_join() {
    let stopping = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let runtime = Runtime::<Bank>::launch_with(Ledger::default(), (), |setup| {
        assembly(setup);
        setup.settings().service("boot", {
            let stopping = stopping.clone();
            let release = release.clone();
            move |mut context: ServiceContext<Bank>| {
                let stopping = stopping.clone();
                let release = release.clone();
                async move {
                    context.ready();
                    context.stopping().await;
                    stopping.notify_one();
                    release.notified().await;
                    Ok::<(), std::io::Error>(())
                }
            }
        });
    })
    .await
    .unwrap();
    service_status(runtime.operations.watch_residents(), "boot", |status| matches!(status, ServiceStatus::Running { .. })).await;
    let first = runtime.operations.shutdown_residents();
    tokio::pin!(first);
    tokio::select! {
        result = &mut first => panic!("cleanup is gated: {result:?}"),
        _ = stopping.notified() => {}
    }
    let second = runtime.operations.shutdown_residents();
    tokio::pin!(second);
    assert!(futures_util::poll!(&mut first).is_pending());
    assert!(futures_util::poll!(&mut second).is_pending());
    release.notify_one();
    let (first, second) = tokio::join!(first, second);
    first.unwrap();
    second.unwrap();
    assert_eq!(
        runtime.operations.resident(&harmos_runtime::operations::resident::Key::root("boot")).and_then(|item| item.health),
        Some(ServiceStatus::Stopped)
    );
    runtime.shutdown().await.unwrap();
}
