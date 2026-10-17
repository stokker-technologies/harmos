/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

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
use harmos::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::{Error, Runtime, services::Fault, services::Service, services::ServiceStatus};
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
            restart: RestartPolicy::Never,
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
    ) -> Result<harmos::operations::Response<usize>, Self::Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move { Ok(42) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn service_status(
    mut services: tokio::sync::watch::Receiver<
        BTreeMap<harmos::operations::resident::Key, harmos::operations::resident::Snapshot>,
    >,
    name: &str,
    wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    services
        .wait_for(|all| {
            all.get(&harmos::operations::resident::Key::root(name))
                .and_then(|item| item.health.as_ref())
                .is_some_and(&wanted)
        })
        .await
        .expect("the runtime keeps the status sender open")
        .get(&harmos::operations::resident::Key::root(name))
        .and_then(|item| item.health.clone())
        .expect("the matched status is present")
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

#[tokio::test]
async fn handle_shares_manager() {
    let runtime = assembly()
        .service("boot", resident)
        .runtime()
        .await
        .unwrap();
    let manager = runtime.handle().operations;
    manager
        .register_service(
            harmos::services::ServiceDefinition {
                id: "dynamic".into(),
                policy: harmos::services::POLICY,
            },
            |_: Arc<()>| resident,
        )
        .unwrap();
    let dynamic = manager.submit_resident("dynamic").await.unwrap();
    assert_eq!(runtime.operations.residents().len(), 2);
    assert_eq!(
        runtime.operations.resident_key(dynamic.id()),
        Some(harmos::operations::resident::Key::root("dynamic"))
    );
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn duplicate_service_refused() {
    let runtime = assembly()
        .service("boot", resident)
        .runtime()
        .await
        .unwrap();
    assert!(matches!(
        runtime.operations.register_service(
            harmos::services::ServiceDefinition {
                id: "boot".into(),
                policy: harmos::services::POLICY
            },
            |_: Arc<()>| resident
        ),
        Err(Error::AlreadyRegistered { domain: "service", name }) if name == "boot"
    ));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn shutdown_preserves_work() {
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
    assert_eq!(
        *runtime
            .operations
            .call_as::<PingContract>(())
            .await
            .unwrap(),
        42
    );
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn admission_rejects_reserved() {
    let runtime = assembly().runtime().await.unwrap();
    for name in ["", "artifact"] {
        assert!(matches!(
            runtime.operations.register_service(
                harmos::services::ServiceDefinition {
                    id: name.into(),
                    policy: harmos::services::POLICY
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_join_started() {
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
                harmos::services::ServiceDefinition {
                    id: id.clone(),
                    policy: harmos::services::POLICY,
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
}

#[tokio::test]
async fn concurrent_shutdowns_join() {
    let stopping = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let runtime = assembly()
        .service("boot", {
            let stopping = stopping.clone();
            let release = release.clone();
            move |mut context: Service<Bank>| {
                let stopping = stopping.clone();
                let release = release.clone();
                async move {
                    context.ready();
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
    service_status(runtime.operations.watch_residents(), "boot", |status| {
        matches!(status, ServiceStatus::Running { .. })
    })
    .await;
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
        runtime
            .operations
            .resident(&harmos::operations::resident::Key::root("boot"))
            .and_then(|item| item.health),
        Some(ServiceStatus::Stopped)
    );
    runtime.stop().await.unwrap();
}

/* === Tests === */
