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
use std::num::NonZeroUsize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::Notify;

// Local Imports
use super::{Lanes, app::App};
use harmos::operations::{
    Context, Operation, OperationPhase, OperationPolicy, RestartPolicy, RetryPolicy, Settings,
};
use harmos::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Request {
    blocked: bool,
    started: Arc<Notify>,
}

struct Hold {
    counts: Arc<Counts>,
}

#[derive(Default)]
struct Counts {
    fail_cleanup: Arc<AtomicBool>,
    constructed: AtomicUsize,
    drops: AtomicUsize,
}

/* === Enums === */
#[harmos::operation(
        request = Request,
        response = (),
        id = "queue.hold",
        policy = OperationPolicy {
            timeout: Duration::from_secs(2),
            attempt_timeout: Duration::from_secs(1),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct HoldContract;

#[harmos::catalog]
enum Operations {
    HoldContract(HoldContract),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Hold {
    type Declaration = HoldContract;
    type Resources = Counts;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    type Error = std::io::Error;

    async fn initialize<'ctx>(
        counts: Arc<Counts>,
        _: Self::Context<'ctx>,
    ) -> Result<Self, Self::Error> {
        counts.constructed.fetch_add(1, Ordering::SeqCst);
        Ok(Self { counts })
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Request,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<()>, Self::Error> {
        let result: ::core::result::Result<(), Self::Error> = async move {
            request.started.notify_one();
            if request.blocked {
                std::future::pending::<()>().await;
            }
            Ok(())
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        if self.counts.fail_cleanup.load(Ordering::SeqCst) {
            Err(std::io::Error::other("retained queued resource"))
        } else {
            Ok(())
        }
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.counts.drops.fetch_add(1, Ordering::SeqCst);
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> Runtime<App> {
    Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(Settings {
            queued: NonZeroUsize::new(1).unwrap(),
            lifecycle_queued: NonZeroUsize::new(1).unwrap(),
            ..Settings::default()
        })
        .runtime()
        .await
        .unwrap()
}

fn request(blocked: bool) -> Request {
    Request {
        blocked,
        started: Arc::new(Notify::new()),
    }
}

/* === Tests === */

pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("queue_refuses_construction", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let counts = Arc::new(Counts::default());
                    runtime
                        .operations
                        .register_operation::<Hold>("app", Lanes::Serial, counts.clone())
                        .unwrap();
                    let first_request = request(true);
                    let started = first_request.started.clone();
                    let first = runtime.operations.submit(first_request).unwrap();
                    started.notified().await;
                    let second = runtime.operations.submit(request(true)).unwrap();
                    let first_id = first.id();
                    let second_id = second.id();
                    assert!(matches!(
                        runtime.operations.submit(request(false)),
                        Err(Error::ResourceExhausted {
                            resource: "operation admission queue"
                        })
                    ));
                    // A queued instance is constructed only when it initializes.
                    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
                    assert_eq!(second.status().phase(), OperationPhase::Queued);
                    tokio::time::timeout(Duration::from_secs(1), runtime.stop())
                        .await
                        .unwrap()
                        .unwrap();
                    for answer in [first.join().await, second.join().await] {
                        assert!(matches!(
                            answer,
                            Err(Error::OperationRefused(Refusal::Cancelled))
                        ));
                    }
                    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
                    assert!(runtime.operations.status(first_id).is_none());
                    assert!(runtime.operations.status(second_id).is_none());
                });
        });
    }
    {
        scenario_runtime::check("cancelled_queue_skips_cleanup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let counts = Arc::new(Counts::default());
                    runtime
                        .operations
                        .register_operation::<Hold>("app", Lanes::Serial, counts.clone())
                        .unwrap();
                    let first_request = request(true);
                    let started = first_request.started.clone();
                    let first = runtime.operations.submit(first_request).unwrap();
                    started.notified().await;
                    let second_request = request(false);
                    counts.fail_cleanup.store(true, Ordering::SeqCst);
                    let mut second = runtime.operations.submit(second_request).unwrap();
                    second.cancel();
                    // A cancelled queued instance never initialized, so it owns
                    // nothing to clean up and releases its queue slot at once.
                    assert!(matches!(
                        (&mut second).await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    second.settle().await.unwrap();
                    assert!(runtime.operations.status(second.id()).is_none());
                    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.drops.load(Ordering::SeqCst), 0);
                    counts.fail_cleanup.store(false, Ordering::SeqCst);
                    let third = runtime.operations.submit(request(false)).unwrap();
                    third.cancel();
                    assert!(matches!(
                        third.join().await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    first.cancel();
                    assert!(matches!(
                        first.join().await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("instances_leave_lookup", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.start_paused(true);
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let counts = Arc::new(Counts::default());
                    runtime
                        .operations
                        .register_operation::<Hold>("app", Lanes::Serial, counts.clone())
                        .unwrap();
                    for _ in 0..64 {
                        let instance = runtime.operations.submit(request(false)).unwrap();
                        let id = instance.id();
                        instance.join().await.unwrap();
                        assert!(runtime.operations.status(id).is_none());
                    }
                    assert_eq!(counts.drops.load(Ordering::SeqCst), 64);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("submission_uses_boot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = runtime().await;
                    let counts = Arc::new(Counts::default());
                    runtime
                        .operations
                        .register_operation::<Hold>("app", Lanes::Serial, counts.clone())
                        .unwrap();
                    let operations = runtime.operations.clone();
                    let instance = std::thread::spawn(move || operations.submit(request(false)))
                        .join()
                        .unwrap()
                        .unwrap();
                    instance.join().await.unwrap();
                    assert_eq!(counts.constructed.load(Ordering::SeqCst), 1);
                    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
}
