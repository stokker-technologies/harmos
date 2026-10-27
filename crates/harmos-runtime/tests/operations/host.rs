/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The in-process binding of the portable host, resident activation
// through it, and the built-in status, cancel and watch operations it shares
// with the remote bindings.

/* === Modules === */

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
use harmos_runtime::builtin;
use harmos_runtime::operations::instance::{Outcome, Phase};
use harmos_runtime::operations::{Host, Instance, Operation, OperationDeclaration, OperationPolicy, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
    timeout: Duration::from_secs(5),
    attempt_timeout: Duration::from_secs(5),
    retries: RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The declaration is its own request, so `call` and `submit` infer it.
#[harmos_runtime::operation(request = Hold, infer_request, id = "fixture_hold",  response = u64, policy = POLICY)]
struct HoldDeclaration;

#[harmos_runtime::message]

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

// A resident service activated through the host by its identity.
#[harmos_runtime::operation(request = Ticker, infer_request,
    id = "fixture_ticker",

    response = (),
    policy = POLICY
)]
struct TickerDeclaration;

#[harmos_runtime::message]

struct Ticker {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Operation for Holding {
    type Request = Hold;
    type Response = <HoldDeclaration as harmos_runtime::OperationDeclaration>::Response;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Error> {
        let gate = context.dependency::<Gate>().expect("fixture registers its dependency");
        Ok(Self(gate))
    }

    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut Hold, _: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<u64>, Error> {
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
    let runtime = Runtime::<App>::builder((), ()).register(Lanes::catalog()).runtime().await.unwrap();
    runtime
        .operations
        .register(harmos_runtime::RegistrationExt::lane(
            harmos_runtime::RegistrationBatch::new({
                let mut dependencies = harmos_runtime::Dependencies::new();
                dependencies.insert(gate.clone()).unwrap();
                dependencies
            })
            .implementation::<HoldDeclaration, Holding>(),
            Lanes::Open,
        ))
        .await
        .unwrap();
    runtime.operations.register(harmos_runtime::RegistrationExt::lane(builtin::Catalog::registration(), Lanes::Open)).await.unwrap();
    (runtime, gate)
}

// Submit Through Only The Portable Host And Instance Contracts.
async fn submitted<H: Host>(host: &H, request: Hold) -> Result<impl Instance<HoldDeclaration, Error = H::Error>, H::Error> {
    host.submit(request).await
}

// The built-ins refuse unknown and foreign instances alike.
fn hidden(error: Option<Error>) -> bool {
    let Some(Error::OperationFailed { source, .. }) = error else {
        return false;
    };
    matches!(source.downcast_ref::<harmos_operations::Error>(), Some(harmos_operations::Error::UnknownInstance(_)))
}

/* === Tests === */

#[tokio::test]
async fn host_instances_settle() {
    {
        let (runtime, gate) = runtime().await;
        let instance = submitted(&runtime.operations, Hold { value: 21 }).await.unwrap();
        gate.started.notified().await;
        let status = instance.status().await.unwrap();
        assert_eq!(status.phase, Phase::Running);
        assert_eq!(status.outcome, None);
        gate.release.add_permits(1);
        let response = instance.join().await.unwrap();
        assert_eq!(*response, 42);
        let called = Host::call::<HoldDeclaration>(&runtime.operations, Hold { value: 4 });
        gate.release.add_permits(1);
        assert_eq!(*called.await.unwrap(), 8);
        assert_eq!(gate.completed.load(Ordering::SeqCst), 2);
        runtime.stop().await.unwrap();
    }
    {
        let (runtime, gate) = runtime().await;
        let instance = submitted(&runtime.operations, Hold { value: 1 }).await.unwrap();
        gate.started.notified().await;
        assert!(instance.cancel_and_join().await.is_err());
        assert_eq!(gate.completed.load(Ordering::SeqCst), 0);
        assert_eq!(gate.cleaned.load(Ordering::SeqCst), 1);
        runtime.stop().await.unwrap();
    }
    {
        let (runtime, gate) = runtime().await;
        let instance = runtime.operation(Hold { value: 1 }).start().await.unwrap();
        gate.started.notified().await;
        let cleaned = gate.cleanup.notified();
        drop(instance);
        // Dropping an unsettled instance cancels it.
        cleaned.await;
        assert_eq!(gate.completed.load(Ordering::SeqCst), 0);
        runtime.stop().await.unwrap();
    }
    {
        let (runtime, gate) = runtime().await;
        let instance = runtime.operation(Hold { value: 1 }).start().await.unwrap();
        gate.started.notified().await;
        let id = Instance::detach(instance);
        let status = runtime.operation(builtin::StatusRequest::of(id)).run().await;
        assert_eq!(status.unwrap().phase, Phase::Running.as_str());
        let cleaned = gate.cleanup.notified();
        gate.release.add_permits(1);
        cleaned.await;
        assert_eq!(gate.completed.load(Ordering::SeqCst), 1);
        runtime.stop().await.unwrap();
    }
}

#[tokio::test]
async fn builtin_controls_instance() {
    let (runtime, gate) = runtime().await;
    let operations = &runtime.operations;
    let mut instance = operations.submit::<HoldDeclaration>(Hold { value: 1 }).await.unwrap();
    gate.started.notified().await;
    let id = instance.id();
    let status = operations.call::<builtin::Status>(builtin::StatusRequest::of(id)).await.unwrap();
    assert_eq!(status.phase, Phase::Running.as_str());
    assert_eq!(status.outcome, "");
    assert!(!status.cancellation_requested);
    let cancelled = operations.call::<builtin::Cancel>(builtin::CancelRequest::of(id)).await.unwrap();
    assert!(cancelled.cancellation_requested);
    assert!(matches!((&mut instance).await, Err(Error::OperationRefused(Refusal::Cancelled))));
    instance.settle().await.unwrap();
    // Settled Instances Leave The Index; Unknown Ids Stay Hidden
    let settled = operations.call::<builtin::Status>(builtin::StatusRequest::of(id)).await;
    assert!(hidden(settled.err()));
    let unknown = harmos_runtime::operations::OperationInstanceId::new(u64::MAX);
    let refused = operations.call::<builtin::Cancel>(builtin::CancelRequest::of(unknown)).await;
    assert!(hidden(refused.err()));
    let status = harmos_runtime::operations::instance::Status::try_from(&*cancelled);
    assert_ne!(status.unwrap().outcome, Some(Outcome::Succeeded));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn resident_leases_watched() {
    let (runtime, gate) = runtime().await;
    let operations = &runtime.operations;
    operations
        .register_service::<App, Gate, _, _>(harmos_runtime::services::ServiceDefinition::new(TickerDeclaration::ID), |gate: Arc<Gate>| {
            move |mut context: harmos_runtime::services::ServiceContext<App>| {
                let gate = gate.clone();
                async move {
                    context.ready();
                    context.stopping().await;
                    gate.cleaned.fetch_add(1, Ordering::SeqCst);
                    Ok::<(), std::io::Error>(())
                }
            }
        })
        .unwrap();

    // Activations Over The Same Resources Share One Instance
    let first = operations.submit_with_resources_at(TickerDeclaration::ID, gate.clone()).await.unwrap();
    let second = operations.submit_with_resources_at(TickerDeclaration::ID, gate.clone()).await.unwrap();
    assert_eq!(first.id(), second.id());
    first.ready().await.unwrap();
    let id = first.id();
    let key = operations.resident_key(id).unwrap();
    assert_eq!(operations.resident(&key).unwrap().leases, 2);
    let status = operations.call::<builtin::Status>(builtin::StatusRequest::of(id)).await.unwrap();
    assert_eq!(status.phase, Phase::Running.as_str());

    // A Watch Streams Every Change Until The Instance Stops
    let settings = harmos_runtime::operations::StreamSettings {
        items: std::num::NonZeroUsize::new(16).unwrap(),
        bytes: std::num::NonZeroUsize::new(4096).unwrap(),
    };
    let mut watch = operations.download::<builtin::Watch>(builtin::WatchRequest::of(id), settings).unwrap();
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
    assert_eq!(phases.last().map(String::as_str), Some(Phase::Stopped.as_str()));
    assert!(phases.iter().any(|phase| phase == Phase::Stopping.as_str()));
    second.settle().await.unwrap();
    assert_eq!(gate.cleaned.load(Ordering::SeqCst), 1);

    // A Stopped Resident Leaves The Index Like A Settled Request
    let stopped = operations.call::<builtin::Status>(builtin::StatusRequest::of(id)).await;
    assert!(hidden(stopped.err()));
    runtime.stop().await.unwrap();
}
