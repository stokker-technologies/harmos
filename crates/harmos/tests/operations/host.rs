/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The in-process binding of the portable host capability, resident activation
// through it, and the built-in status, cancel and watch operations it shares
// with the remote bindings.

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// Third Party Imports
use tokio::sync::{Notify, Semaphore};

// Local Imports
use super::{Lanes, app::App};
use harmos::builtin;
use harmos::operations::instance::{Outcome, Phase};
use harmos::operations::{
    Context, Host, Instance, Operation, OperationDeclaration, OperationPolicy, RestartPolicy,
    RetryPolicy,
};
use harmos::{Error, Refusal, Runtime};

/* === Constants === */
const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(5),
    attempt_timeout: Duration::from_secs(5),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// The declaration is its own request, so `call` and `submit` infer it.
#[harmos::message]
struct Hold {
    value: u64,
}

struct Gate {
    started: Notify,
    release: Semaphore,
    completed: AtomicUsize,
    cleaned: AtomicUsize,
    cleanup: Notify,
}

struct Holding(Arc<Gate>);

// A resident service activated through the host capability by its identity.
struct Ticker;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for Hold {
    type Request = Self;
    type Response = u64;
    const ID: &'static str = "capability.hold";
    const POLICY: OperationPolicy = POLICY;
}

impl OperationDeclaration for Ticker {
    type Request = ();
    type Response = ();
    const ID: &'static str = "capability.ticker";
    const POLICY: OperationPolicy = harmos::services::POLICY;
}

impl Operation for Holding {
    type Declaration = Hold;
    type Resources = Gate;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = ();

    async fn initialize<'ctx>(gate: Arc<Gate>, _: Self::Context<'ctx>) -> Result<Self, Error> {
        Ok(Self(gate))
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        request: &mut Hold,
        _: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<u64>, Error> {
        self.0.started.notify_one();
        self.0.release.acquire().await.unwrap().forget();
        self.0.completed.fetch_add(1, Ordering::SeqCst);
        Ok((request.value * 2).into())
    }

    async fn cleanup(&mut self) -> Result<(), Error> {
        self.0.cleaned.fetch_add(1, Ordering::SeqCst);
        self.0.cleanup.notify_one();
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> (Runtime<App>, Arc<Gate>) {
    let gate = Arc::new(Gate {
        started: Notify::new(),
        release: Semaphore::new(0),
        completed: AtomicUsize::new(0),
        cleaned: AtomicUsize::new(0),
        cleanup: Notify::new(),
    });
    let runtime = Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .runtime()
        .await
        .unwrap();
    runtime
        .operations
        .register_operation::<Holding>("app", Lanes::Open, gate.clone())
        .unwrap();
    builtin::Catalog::register(&runtime.operations, "host", Lanes::Open).unwrap();
    (runtime, gate)
}

// Generic over the capability, as a body shared by every binding would be.
async fn joined<H: Host>(host: &H, gate: &Gate) -> Result<u64, H::Error> {
    let instance = host.submit(Hold { value: 21 }).await?;
    gate.started.notified().await;
    let status = instance.status().await?;
    assert_eq!(status.phase, Phase::Running);
    assert_eq!(status.outcome, None);
    gate.release.add_permits(1);
    let response = instance.join().await?;
    Ok(*response)
}

async fn cancelled<H: Host>(host: &H, gate: &Gate) -> Result<(), H::Error> {
    let instance = host.submit_as::<Hold>(Hold { value: 1 }).await?;
    gate.started.notified().await;
    assert!(instance.cancel_and_join().await.is_err());
    Ok(())
}

// The built-ins refuse unknown and foreign instances alike.
fn hidden(error: Option<Error>) -> bool {
    let Some(Error::OperationFailed { source, .. }) = error else {
        return false;
    };
    matches!(
        source.downcast_ref::<Error>(),
        Some(Error::NotRegistered { .. })
    )
}

/* === Tests === */
pub(super) fn contract_scenario() {
    {
        scenario_runtime::check("host_submit_joins", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (runtime, gate) = runtime().await;
                    assert_eq!(joined(&runtime.operations, &gate).await.unwrap(), 42);
                    let called = Host::call(&runtime.operations, Hold { value: 4 });
                    gate.release.add_permits(1);
                    assert_eq!(*called.await.unwrap(), 8);
                    assert_eq!(gate.completed.load(Ordering::SeqCst), 2);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("host_cancel_and_join", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (runtime, gate) = runtime().await;
                    cancelled(&runtime.operations, &gate).await.unwrap();
                    assert_eq!(gate.completed.load(Ordering::SeqCst), 0);
                    assert_eq!(gate.cleaned.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("host_drop_cancels", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (runtime, gate) = runtime().await;
                    let instance = runtime.operations.submit(Hold { value: 1 }).unwrap();
                    gate.started.notified().await;
                    let cleaned = gate.cleanup.notified();
                    drop(instance);
                    tokio::time::timeout(Duration::from_secs(2), cleaned)
                        .await
                        .expect("dropping an unsettled instance cancels it");
                    assert_eq!(gate.completed.load(Ordering::SeqCst), 0);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("host_detach_continues", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (runtime, gate) = runtime().await;
                    let instance = runtime.operations.submit(Hold { value: 1 }).unwrap();
                    gate.started.notified().await;
                    let id = Instance::detach(instance);
                    let status = runtime.operations.call(builtin::Status::of(id)).await;
                    assert_eq!(status.unwrap().phase, Phase::Running.as_str());
                    let cleaned = gate.cleanup.notified();
                    gate.release.add_permits(1);
                    tokio::time::timeout(Duration::from_secs(2), cleaned)
                        .await
                        .expect("a detached instance runs to completion");
                    assert_eq!(gate.completed.load(Ordering::SeqCst), 1);
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("builtin_status_and_cancel", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (runtime, gate) = runtime().await;
                    let operations = &runtime.operations;
                    let mut instance = operations.submit(Hold { value: 1 }).unwrap();
                    gate.started.notified().await;
                    let id = instance.id();
                    let status = operations.call(builtin::Status::of(id)).await.unwrap();
                    assert_eq!(status.phase, Phase::Running.as_str());
                    assert_eq!(status.outcome, "");
                    assert!(!status.cancellation_requested);
                    let cancelled = operations.call(builtin::Cancel::of(id)).await.unwrap();
                    assert!(cancelled.cancellation_requested);
                    assert!(matches!(
                        (&mut instance).await,
                        Err(Error::OperationRefused(Refusal::Cancelled))
                    ));
                    instance.settle().await.unwrap();
                    // Settled Instances Leave The Index; Unknown Ids Stay Hidden
                    let settled = operations.call(builtin::Status::of(id)).await;
                    assert!(hidden(settled.err()));
                    let unknown = harmos::operations::OperationInstanceId::new(u64::MAX);
                    let refused = operations.call(builtin::Cancel::of(unknown)).await;
                    assert!(hidden(refused.err()));
                    let status = harmos::operations::instance::Status::try_from(&*cancelled);
                    assert_ne!(status.unwrap().outcome, Some(Outcome::Succeeded));
                    runtime.stop().await.unwrap();
                });
        });
    }
    {
        scenario_runtime::check("resident_lease_watch", || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (runtime, gate) = runtime().await;
                    let operations = &runtime.operations;
                    operations
                        .register_service::<App, Gate, _, _>(
                            harmos::services::ServiceDefinition::new(Ticker::ID),
                            |gate: Arc<Gate>| {
                                move |mut context: harmos::services::Service<App>| {
                                    let gate = gate.clone();
                                    async move {
                                        context.ready();
                                        context.stopping().await;
                                        gate.cleaned.fetch_add(1, Ordering::SeqCst);
                                        Ok::<(), harmos::services::Fault<&str>>(())
                                    }
                                }
                            },
                        )
                        .unwrap();

                    // Activations Over The Same Resources Share One Instance
                    let first = operations
                        .submit_with_resources::<Ticker, Gate>((), gate.clone())
                        .await
                        .unwrap();
                    let second = operations
                        .submit_with_resources::<Ticker, Gate>((), gate.clone())
                        .await
                        .unwrap();
                    assert_eq!(first.id(), second.id());
                    first.ready().await.unwrap();
                    let id = first.id();
                    let key = operations.resident_key(id).unwrap();
                    assert_eq!(operations.resident(&key).unwrap().leases, 2);
                    let status = operations.call(builtin::Status::of(id)).await.unwrap();
                    assert_eq!(status.phase, Phase::Running.as_str());

                    // A Watch Streams Every Change Until The Instance Stops
                    let settings = harmos::operations::StreamSettings {
                        items: std::num::NonZeroUsize::new(16).unwrap(),
                        bytes: std::num::NonZeroUsize::new(4096).unwrap(),
                    };
                    let mut watch = operations
                        .download::<builtin::Watch>(builtin::Watch::of(id), settings)
                        .unwrap();
                    let current = watch.next().await.unwrap().unwrap();
                    assert_eq!(current.phase, Phase::Running.as_str());
                    drop(first);
                    assert_eq!(operations.resident(&key).unwrap().leases, 1);
                    assert_eq!(gate.cleaned.load(Ordering::SeqCst), 0);
                    second.cancel();
                    let mut phases = Vec::new();
                    while let Some(state) = watch.next().await.unwrap() {
                        phases.push(state.phase.clone());
                    }
                    let last = watch.join().await.unwrap();
                    assert_eq!(last.phase, Phase::Stopped.as_str());
                    assert_eq!(
                        phases.last().map(String::as_str),
                        Some(Phase::Stopped.as_str())
                    );
                    assert!(phases.iter().any(|phase| phase == Phase::Stopping.as_str()));
                    second.settle().await.unwrap();
                    assert_eq!(gate.cleaned.load(Ordering::SeqCst), 1);

                    // A Stopped Resident Leaves The Index Like A Settled Request
                    let stopped = operations.call(builtin::Status::of(id)).await;
                    assert!(hidden(stopped.err()));
                    runtime.stop().await.unwrap();
                });
        });
    }
}
