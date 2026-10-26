/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runs one service activation as a resident instance. The resident is created
// inside the instance's owned attempt, driven by the restart policy, and its
// cleanup stays owned by the instance until it settles.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::TypeId;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
// Third Party Imports

// Local Imports
use super::{Resident, ServiceContext, ServiceDefinition, ServiceExit};
use crate::operations::OperationPhase;
use crate::operations::resident::{Completion, Outcome, Report, Resources, Runner, Settlement, Start};
use crate::supervision::{self, Task, instance::Instance};
use crate::{Application, Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Created<A> = Result<Option<Box<dyn ErasedResident<A>>>>;

/* === Structs === */

// A service whose resident each activation creates from shared resources.
pub(crate) struct Typed<A: Application, R, F> {
    definition: ServiceDefinition,
    factory: F,
    marker: PhantomData<fn(R) -> A>,
}

// A service whose one resident runs once; it is never reactivated.
pub(crate) struct Once<A: Application> {
    definition: ServiceDefinition,
    resident: Mutex<Option<Box<dyn ErasedResident<A>>>>,
}

struct Physical<R> {
    resident: R,
    stopped: bool,
    lifecycle: Option<Arc<Instance>>,
    report: Option<Report>,
}

pub(super) struct Failure {
    pub(super) message: String,
    pub(super) source: Arc<dyn std::error::Error + Send + Sync>,
}

pub(super) struct Cleanup<A: Application> {
    pub(super) lifecycle: Arc<Instance>,
    resident: OnceLock<supervision::cleanup::Cleanup<Box<dyn ErasedResident<A>>>>,
    pub(super) finalized: AtomicBool,
    logical: Mutex<Option<Outcome>>,
}

/* === Enums === */

/* === Traits === */

pub(super) trait ErasedResident<A: Application>: Send {
    fn bind_lifecycle(&mut self, lifecycle: Arc<Instance>, report: Report);

    fn run(&mut self, context: ServiceContext<A>) -> Pin<Box<dyn Future<Output = std::result::Result<(), Failure>> + Send + '_>>;

    fn cleanup(&mut self) -> Pin<Box<dyn Future<Output = std::result::Result<(), Failure>> + Send + '_>>;
}

trait Create<A: Application>: Send + Sync + 'static {
    fn definition(&self) -> &ServiceDefinition;
    fn create(&self, resources: Resources, admitted: bool) -> Created<A>;
}

/* === Implementations === */

impl<A, R, F, T> Typed<A, R, F>
where
    A: Application,
    R: Send + Sync + 'static,
    F: Fn(Arc<R>) -> T + Send + Sync + 'static,
    T: Resident<A>,
{
    pub(crate) fn new(definition: ServiceDefinition, factory: F) -> Self {
        Self { definition, factory, marker: PhantomData }
    }
}

impl<A: Application> Once<A> {
    pub(crate) fn new<R: Resident<A>>(definition: ServiceDefinition, resident: R) -> Self {
        Self { definition, resident: Mutex::new(Some(Box::new(Physical::new(resident)))) }
    }

    pub(crate) fn id(&self) -> &str {
        &self.definition.id
    }

    pub(crate) fn definition(&self) -> ServiceDefinition {
        self.definition.clone()
    }
}

impl<A, R, F, T> Create<A> for Typed<A, R, F>
where
    A: Application,
    R: Send + Sync + 'static,
    F: Fn(Arc<R>) -> T + Send + Sync + 'static,
    T: Resident<A>,
{
    fn definition(&self) -> &ServiceDefinition {
        &self.definition
    }

    fn create(&self, resources: Resources, admitted: bool) -> Created<A> {
        if !admitted {
            return Ok(None);
        }
        let resources = resources
            .downcast::<R>()
            .map_err(|_| Error::InvalidInput { field: "service scope", reason: "resource type differs from the registered factory" })?;
        Ok(Some(Box::new(Physical::new((self.factory)(resources)))))
    }
}

impl<A: Application> Create<A> for Once<A> {
    fn definition(&self) -> &ServiceDefinition {
        &self.definition
    }

    fn create(&self, _: Resources, _: bool) -> Created<A> {
        let resident = self
            .resident
            .lock()
            .expect("service resident is healthy")
            .take()
            .ok_or(Error::NotReady { domain: "retired assembly service" })?;
        Ok(Some(resident))
    }
}

impl<A, R, F, T> Runner for Typed<A, R, F>
where
    A: Application,
    R: Send + Sync + 'static,
    F: Fn(Arc<R>) -> T + Send + Sync + 'static,
    T: Resident<A>,
{
    fn resources(&self) -> TypeId {
        TypeId::of::<R>()
    }

    fn spawn(self: Arc<Self>, start: Start) -> (Task<Outcome>, Arc<dyn Settlement>) {
        spawn(self, start)
    }
}

impl<A: Application> Runner for Once<A> {
    fn resources(&self) -> TypeId {
        TypeId::of::<()>()
    }

    fn reactivates(&self) -> bool {
        false
    }

    fn spawn(self: Arc<Self>, start: Start) -> (Task<Outcome>, Arc<dyn Settlement>) {
        spawn(self, start)
    }
}

impl<R> Physical<R> {
    fn new(resident: R) -> Self {
        Self { resident, stopped: false, lifecycle: None, report: None }
    }

    fn transition(&self, phase: OperationPhase) {
        self.lifecycle.as_ref().expect("resident bound before execution").transition(phase);
        if let Some(report) = &self.report {
            report.publish();
        }
    }
}

impl<A, R> ErasedResident<A> for Physical<R>
where
    A: Application,
    R: Resident<A>,
{
    fn bind_lifecycle(&mut self, lifecycle: Arc<Instance>, report: Report) {
        self.lifecycle = Some(lifecycle);
        self.report = Some(report);
    }

    fn run(&mut self, context: ServiceContext<A>) -> Pin<Box<dyn Future<Output = std::result::Result<(), Failure>> + Send + '_>> {
        Box::pin(async move {
            self.stopped = false;
            Resident::run(&mut self.resident, context)
                .await
                .map_err(|error| Failure { message: error.to_string(), source: Arc::new(super::FailureSource(error)) })
        })
    }

    fn cleanup(&mut self) -> Pin<Box<dyn Future<Output = std::result::Result<(), Failure>> + Send + '_>> {
        Box::pin(async move {
            if !self.stopped {
                self.transition(OperationPhase::Stopping);
                Resident::stop(&mut self.resident)
                    .await
                    .map_err(|error| Failure { message: error.to_string(), source: Arc::new(super::FailureSource(error)) })?;
                self.stopped = true;
            }
            self.transition(OperationPhase::Finalizing);
            Resident::cleanup(&mut self.resident)
                .await
                .map_err(|error| Failure { message: error.to_string(), source: Arc::new(super::FailureSource(error)) })
        })
    }
}

impl<A: Application> supervision::cleanup::Resource for Box<dyn ErasedResident<A>> {
    fn finalize(&mut self) -> supervision::cleanup::Finalize<'_> {
        Box::pin(async move { cleanup(self.as_mut()).await.map_err(Arc::new) })
    }
}

impl<A: Application> Cleanup<A> {
    pub(super) fn record(&self, outcome: Outcome) {
        let mut logical = self.logical.lock().expect("service outcome is healthy");
        if logical.is_some() {
            return;
        }
        let state = match &outcome {
            Ok(()) if self.lifecycle.status().cancellation_requested() => supervision::instance::Outcome::Cancelled,
            Ok(()) => supervision::instance::Outcome::Succeeded,
            Err(error) => match error.as_ref() {
                Error::TaskFailed { source, .. } if matches!(source.downcast_ref::<ServiceExit>(), Some(ServiceExit::StopTimedOut)) => {
                    supervision::instance::Outcome::TimedOut
                }
                _ => supervision::instance::Outcome::Failed,
            },
        };
        *logical = Some(outcome);
        self.lifecycle.complete(state);
    }
}

impl<A: Application> Settlement for Cleanup<A> {
    fn lifecycle(&self) -> &Arc<Instance> {
        &self.lifecycle
    }

    fn record(&self, outcome: &Outcome) {
        Cleanup::record(self, outcome.clone());
        if !self.lifecycle.is_settled()
            && let Err(error) = outcome
        {
            self.lifecycle.interrupted(error.clone());
        }
    }

    fn retry(self: Arc<Self>, report: Report) -> Option<Completion> {
        if self.lifecycle.is_settled() {
            return None;
        }
        Some(Box::pin(async move {
            self.lifecycle.transition(OperationPhase::Finalizing);
            let result = match self.resident.get() {
                Some(resident) if self.finalized.load(Ordering::Acquire) => resident.release().await,
                Some(resident) => resident.settle().await,
                None => Ok(()),
            };
            let outcome = match result {
                Ok(()) => {
                    self.finalized.store(true, Ordering::Release);
                    self.lifecycle.finished();
                    self.logical.lock().expect("service outcome is healthy").clone().unwrap_or(Ok(()))
                }
                Err(error) => Err(error),
            };
            report.complete(&outcome);
            outcome
        }))
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn spawn<A: Application, C: Create<A>>(create: Arc<C>, start: Start) -> (Task<Outcome>, Arc<dyn Settlement>) {
    let Start { resources, lifecycle, stopping, report, operations } = start;
    let cleanup =
        Arc::new(Cleanup::<A> { lifecycle, resident: OnceLock::new(), finalized: AtomicBool::new(true), logical: Mutex::new(None) });
    let retained = Arc::clone(&cleanup);
    let task = Task::with_cancellation(stopping, move |stopping| async move {
        let owned = Arc::clone(&retained);
        let attempt = report.clone();
        let outcome = supervision::joined(async move {
            let admitted = owned.lifecycle.initialize_attempt(0);
            attempt.publish();
            let Some(mut resident) = create.create(resources, admitted)? else {
                return Ok(());
            };
            resident.bind_lifecycle(Arc::clone(&owned.lifecycle), attempt.clone());
            assert!(
                owned.resident.set(supervision::cleanup::Cleanup::new(resident)).is_ok(),
                "a service activation creates one physical resident"
            );
            let mut original =
                owned.resident.get().expect("created resident is retained").take().expect("the service instance owns its resident");
            owned.finalized.store(false, Ordering::Release);
            super::restart::run(create.definition(), original.resource().as_mut(), &owned, &attempt, &operations, stopping).await
        })
        .await
        .map_err(|source| Error::TaskFailed { domain: "service supervisor", source: Arc::new(source) })
        .and_then(|answer| answer)
        .map_err(Arc::new);
        retained.record(outcome.clone());
        let outcome = match retained.resident.get() {
            Some(resident) if retained.finalized.load(Ordering::Acquire) => match resident.release().await {
                Ok(()) => {
                    retained.lifecycle.finished();
                    outcome
                }
                Err(error) => Err(error),
            },
            None => {
                retained.lifecycle.finished();
                outcome
            }
            Some(_) => outcome,
        };
        report.complete(&outcome);
        outcome
    });
    (task, cleanup)
}

pub(super) async fn cleanup<A: Application>(resident: &mut dyn ErasedResident<A>) -> Result<()> {
    let mut future = std::pin::pin!(async { resident.cleanup().await });
    std::future::poll_fn(|context| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(poll) => poll.map(|result| result.map_err(|failure| Error::TaskFailed { domain: "service cleanup", source: failure.source })),
        Err(_) => std::task::Poll::Ready(Err(Error::TaskFailed { domain: "service cleanup", source: Arc::new(ServiceExit::Panicked) })),
    })
    .await
}

/* === Tests === */
