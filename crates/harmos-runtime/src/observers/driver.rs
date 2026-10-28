/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runs one observer activation as a resident instance: the backend runs inside
// the instance's owned attempt, and the physical children a remote backend
// submits settle before the instance stops.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::TypeId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::ObserverBackend;
use crate::Error;
#[cfg(feature = "plugin-sidecar")]
use crate::Result;
use crate::operations::resident::{self, Completion, Outcome, Report, Runner, Settlement, Start};
use crate::operations::{OperationInstanceId, OperationPhase};
use crate::supervision::{self, Task, instance::Instance};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct Backend<B>(pub(crate) B);

pub(crate) struct ChildSettlement {
    pending: Mutex<Vec<Arc<Instance>>>,
    lifecycle: Arc<Instance>,
    outcome: Mutex<Option<Outcome>>,
    backend_returned: AtomicBool,
    backend_cleanup: AtomicBool,
}

pub struct ObserverActivation {
    report: Report,
    stopping: watch::Receiver<bool>,

    #[cfg(feature = "plugin-sidecar")]
    children: Arc<ChildSettlement>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<B: ObserverBackend> Runner for Backend<B> {
    fn resources(&self) -> TypeId {
        TypeId::of::<harmos_sdk::ExecutionContext>()
    }

    fn spawn(self: Arc<Self>, start: Start) -> (Task<Outcome>, Arc<dyn Settlement>) {
        let Start {
            resources, lifecycle, stopping, report, ..
        } = start;
        let children = Arc::new(ChildSettlement {
            pending: Mutex::new(Vec::new()),
            lifecycle,
            outcome: Mutex::new(None),
            backend_returned: AtomicBool::new(false),
            backend_cleanup: AtomicBool::new(false),
        });
        let completion = Arc::clone(&children);
        let task = Task::with_cancellation(stopping, move |stopping| async move {
            let admitted = completion.lifecycle.initialize_attempt(0);
            report.publish();
            let activation = ObserverActivation {
                report: report.clone(),
                stopping: stopping.subscribe(),
                #[cfg(feature = "plugin-sidecar")]
                children: Arc::clone(&completion),
            };
            // Calling backend code happens inside the owned attempt,
            // including constructing its future, so panics are joined.
            let outcome = supervision::joined(async move {
                if !admitted {
                    return Ok(());
                }
                let resources = resources.downcast::<harmos_sdk::ExecutionContext>().map_err(|_| Error::InvalidInput {
                    field: "observer scope",
                    reason: "resource type differs from the registered backend",
                })?;
                self.0.run(resources, activation).await
            })
            .await;
            let outcome = match outcome {
                Ok(outcome) => {
                    completion.backend_returned.store(true, Ordering::Release);
                    outcome.map_err(Arc::new)
                }
                Err(source) => Err(Arc::new(Error::TaskFailed {
                    domain: "observer backend",
                    source: Arc::new(source),
                })),
            };
            *completion.outcome.lock().expect("observer outcome is healthy") = Some(outcome.clone());
            if !completion.backend_returned.load(Ordering::Acquire) {
                report.complete(&outcome);
            }
            let outcome = completion.finish(false).await;
            report.complete(&outcome);
            outcome
        });
        (task, children)
    }
}

impl Settlement for ChildSettlement {
    fn lifecycle(&self) -> &Arc<Instance> {
        &self.lifecycle
    }

    fn retry(self: Arc<Self>, report: Report) -> Option<Completion> {
        let retry = self.outcome.lock().expect("observer outcome is healthy").is_some() && !self.lifecycle.is_settled();
        retry.then(|| {
            Box::pin(async move {
                let outcome = self.finish(true).await;
                report.complete(&outcome);
                outcome
            }) as Completion
        })
    }
}

impl ChildSettlement {
    // Retain one physical child of this activation until the activation
    // settles; nothing new is admitted once the backend has returned.
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn submit(
        &self, retirement: bool, submit: impl FnOnce() -> Result<crate::operations::EncodedInvocation>,
    ) -> Result<crate::operations::EncodedInvocation> {
        let mut pending = self.pending.lock().expect("observer children are healthy");
        pending.retain(|control| !control.is_settled());
        if pending.len() >= 64 || (!retirement && self.outcome.lock().expect("observer outcome is healthy").is_some()) {
            return Err(Error::ResourceExhausted { resource: "observer physical children" });
        }
        let invocation = submit()?;
        pending.push(Arc::clone(&invocation.control));
        Ok(invocation)
    }

    // Stop settling a child whose failed cleanup the backend retains itself.
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn forget(&self, control: &Arc<Instance>) {
        self.pending.lock().expect("observer children are healthy").retain(|pending| !Arc::ptr_eq(pending, control));
    }

    async fn finish(&self, recover: bool) -> Outcome {
        let outcome = self
            .outcome
            .lock()
            .expect("observer outcome is healthy")
            .as_ref()
            .expect("backend outcome recorded before child settlement")
            .clone();
        if self.lifecycle.status().outcome().is_none() {
            let terminal = match &outcome {
                Ok(()) if self.lifecycle.status().cancellation_requested() => supervision::instance::Outcome::Cancelled,
                Ok(()) => supervision::instance::Outcome::Succeeded,
                Err(_) => supervision::instance::Outcome::Failed,
            };
            self.lifecycle.complete(terminal);
        }
        if !self.backend_cleanup.load(Ordering::Acquire) {
            self.lifecycle.transition(OperationPhase::Finalizing);
        }
        let pending = self.pending.lock().expect("observer children are healthy").clone();
        if !self.backend_returned.load(Ordering::Acquire) {
            for control in &pending {
                control.cancel();
            }
        }
        let joining = pending
            .into_iter()
            .map(|control| async move { control.settle().await.map_err(|source| Arc::new(Error::OperationSettlement { source })) });
        if let Err(failure) = supervision::settle_all(joining).await {
            self.lifecycle.interrupted(failure.clone());
            return Err(failure);
        }
        self.pending.lock().expect("observer children are healthy").clear();
        if self.backend_returned.load(Ordering::Acquire) {
            if self.backend_cleanup.load(Ordering::Acquire) && !self.lifecycle.is_settled() {
                if !recover {
                    return Err(self.lifecycle.status().cleanup_failure().cloned().expect("retained backend failure"));
                }
                self.lifecycle.settle().await?;
            } else {
                self.lifecycle.finished();
            }
        } else if let Err(failure) = &outcome {
            self.lifecycle.interrupted(failure.clone());
        }
        outcome
    }
}

impl ObserverActivation {
    pub fn key(&self) -> &resident::Key {
        self.report.key()
    }

    pub fn id(&self) -> OperationInstanceId {
        self.report.id()
    }

    // The activation initialized and observes.
    pub fn ready(&self) {
        self.report.transition(OperationPhase::Running);
    }

    // A handle that reports readiness from inside the observing future.
    pub fn readiness(&self) -> impl Fn() + Send + Sync + 'static {
        let report = self.report.clone();
        move || report.transition(OperationPhase::Running)
    }

    // The activation stops: no lease joins it again.
    pub fn begin_stopping(&self) {
        self.report.quiesce();
        self.report.transition(OperationPhase::Stopping);
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn cleaning_up(&self) {
        self.report.transition(OperationPhase::Finalizing);
    }

    pub async fn stopping(&mut self) {
        while !*self.stopping.borrow_and_update() {
            if self.stopping.changed().await.is_err() {
                break;
            }
        }
    }

    // Keep a failed physical cleanup owned by this activation for a later retry.
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn retain<R: supervision::cleanup::Resource>(&self, resource: R, failure: Arc<Error>) {
        self.children.backend_cleanup.store(true, Ordering::Release);
        self.children.lifecycle.retain(resource, failure);
        self.report.publish();
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn depend_on(&self, provider: &Arc<Instance>) -> Result<()> {
        self.children.lifecycle.depend_on(provider, false)
    }

    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn child_settlement(&self) -> Arc<ChildSettlement> {
        Arc::clone(&self.children)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
