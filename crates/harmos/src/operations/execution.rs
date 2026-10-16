/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, watch};

// Local Imports
use super::OperationInstanceId;
use super::finalization::{Finalize, Finalizer, Retained};
use super::instances::{Instances, QueuePermit};
use super::lane::Admission;
use super::scope::{Scope, ScopeCore};
use crate::operations::{FailureKind, OperationPolicy, ReplaySafety};
use crate::supervision::instance::{Instance, Outcome, Phase};
use crate::supervision::{executor, interruptible};
use crate::{Error, Refusal, Result, state::Position};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub(super) type Attempt<'a, O> = Pin<Box<dyn Future<Output = Result<O>> + Send + 'a>>;
type Completed<O> = Box<dyn Fn(&O) + Send>;

/* === Structs === */
pub(crate) struct Prepared<O> {
    pub(super) body: Box<dyn Body<O>>,
    pub(super) policy: OperationPolicy,
    pub(super) replay: ReplaySafety,
    pub(super) id: OperationInstanceId,
    pub(super) completed: Option<Completed<O>>,
    pub(super) refusal: Option<Error>,
    pub(super) middleware: Option<super::middleware::Session>,
}

pub(super) struct Execution {
    pub(super) admission: Admission,
    pub(super) submitted: tokio::time::Instant,
    pub(super) cancelled: watch::Receiver<bool>,
    pub(super) instance: Arc<Instance>,
    pub(super) queue: QueuePermit,
}

struct Budget {
    submitted: tokio::time::Instant,
    timeout: Duration,
}

/* === Enums === */
enum Stage<O> {
    Completed(Result<O>),
    Cancelled,
    Expired,
}

/* === Traits === */
pub(super) trait Body<O>: Send {
    fn transferred(&self) -> bool {
        false
    }

    fn initialize<'a>(&'a mut self, _scope: Scope<'a>) -> Attempt<'a, ()> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn run<'a>(&'a mut self, scope: Scope<'a>) -> Attempt<'a, O>;
    fn classify(&self, _error: &Error) -> FailureKind {
        FailureKind::Permanent
    }

    fn cleanup_phase(&self) -> Phase {
        Phase::Finalizing
    }

    fn finalize(&mut self) -> Finalize<'_> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn retain(
        self: Box<Self>,
        outcome: Option<Error>,
        cause: Arc<Error>,
    ) -> (Box<dyn Finalizer>, Option<Error>, Arc<Error>);
}

/* === Implementations === */
impl Budget {
    fn remaining(&self) -> Duration {
        self.timeout
            .saturating_sub(executor::now().saturating_duration_since(self.submitted))
    }
}

impl<O> Prepared<O> {
    pub(super) fn retain_unavailable(mut self, control: &Instance, admission: Admission) {
        let outcome = self
            .refusal
            .take()
            .or_else(|| Some(Refusal::Unavailable.into()));
        let cause = Arc::new(Error::Closing {
            domain: "preparation driver",
        });
        let (owner, _, diagnostic) = self.body.retain(outcome, cause);
        control.complete(Outcome::Unavailable);
        control.retain(
            Retained {
                owner,
                _permit: None,
                _admission: admission,
            },
            diagnostic,
        );
    }
}

/* === Public Functions === */
pub(super) async fn execute<O: Send + 'static>(
    instances: Arc<Instances>,
    mut prepared: Prepared<O>,
    execution: Execution,
    cause: Option<Position>,
) -> Result<O> {
    let Execution {
        admission,
        submitted,
        mut cancelled,
        instance,
        queue,
    } = execution;
    let budget = Budget {
        submitted,
        timeout: prepared.policy.timeout,
    };
    let mut middleware = prepared.middleware.take();
    let mut refusal = prepared.refusal.take();
    if refusal.is_none()
        && let Some(session) = &mut middleware
    {
        refusal = match bounded(
            budget.remaining(),
            Box::pin(session.enter()),
            &mut cancelled,
        )
        .await
        {
            Stage::Completed(answer) => answer.err(),
            Stage::Cancelled => Some(Refusal::Cancelled.into()),
            Stage::Expired => Some(Refusal::TimedOut.into()),
        };
    }
    let was_refused = refusal.is_some();
    let mut permit = None;
    let answer = match refusal {
        Some(error) => Err(error),
        None => match acquire(&admission, &budget, &mut cancelled).await {
            Err(error) => Err(error),
            Ok(acquired) => {
                permit = Some(acquired);
                queue.release();
                match initialize(
                    &instances,
                    &admission,
                    prepared.body.as_mut(),
                    &mut cancelled,
                    &budget,
                    &instance,
                    cause,
                )
                .await
                {
                    Err(error) => Err(error),
                    Ok(()) => {
                        attempts(
                            &instances,
                            &admission,
                            &mut prepared,
                            &mut cancelled,
                            &budget,
                            &instance,
                            cause,
                        )
                        .await
                    }
                }
            }
        },
    };

    instance.complete(outcome(&answer));
    instance.transition(prepared.body.cleanup_phase());
    if let Err(cleanup) = prepared.body.finalize().await {
        let logical = match answer {
            Err(error) => Some(error),
            Ok(_) if *cancelled.borrow() => Some(Refusal::Cancelled.into()),
            Ok(_) if budget.remaining().is_zero() => Some(Refusal::TimedOut.into()),
            Ok(_) => None,
        };
        instance.complete(logical.as_ref().map_or(Outcome::Succeeded, failure_outcome));
        let (owner, failure, diagnostic) = prepared.body.retain(logical, Arc::new(cleanup));
        instance.retain(
            Retained {
                owner,
                _permit: permit,
                _admission: admission,
            },
            diagnostic,
        );
        let failure = failure.unwrap_or_else(|| Refusal::Unavailable.into());
        if let Some(session) = &mut middleware {
            session.observe(super::middleware::Observation {
                outcome: failure_outcome(&failure),
                settled: false,
            });
        }
        return Err(failure);
    }
    drop(prepared.body);
    let answer = if was_refused {
        answer
    } else if *cancelled.borrow() {
        Err(Refusal::Cancelled.into())
    } else if budget.remaining().is_zero() {
        Err(Refusal::TimedOut.into())
    } else {
        answer
    };
    if let (Ok(response), Some(completed)) = (&answer, prepared.completed) {
        completed(response);
    }
    drop(permit);
    drop(admission);
    instance.complete(outcome(&answer));
    instance.finished();
    if let Some(session) = &mut middleware {
        session.observe(super::middleware::Observation {
            outcome: outcome(&answer),
            settled: true,
        });
    }
    answer
}

/* === Private Functions === */
async fn acquire(
    admission: &Admission,
    budget: &Budget,
    cancelled: &mut watch::Receiver<bool>,
) -> Result<OwnedSemaphorePermit> {
    if *cancelled.borrow() {
        return Err(Refusal::Cancelled.into());
    }
    if budget.remaining().is_zero() {
        return Err(Refusal::TimedOut.into());
    }
    let waiting = interruptible(admission.semaphore().acquire_owned(), cancelled);
    match executor::timeout(budget.remaining(), waiting).await {
        Err(_) => Err(Refusal::TimedOut.into()),
        Ok(None) => Err(Refusal::Cancelled.into()),
        Ok(Some(Err(_))) => Err(Refusal::Unavailable.into()),
        Ok(Some(Ok(permit))) => Ok(permit),
    }
}

async fn initialize<O: Send + 'static>(
    instances: &Arc<Instances>,
    admission: &Admission,
    body: &mut dyn Body<O>,
    cancelled: &mut watch::Receiver<bool>,
    budget: &Budget,
    instance: &Arc<Instance>,
    cause: Option<Position>,
) -> Result<()> {
    instance.transition(Phase::Initializing);
    let core = ScopeCore::new(
        instances.clone(),
        admission.clone(),
        cause,
        Arc::downgrade(instance),
    );
    let answer = bounded(
        budget.remaining(),
        body.initialize(Scope::over(&core)),
        cancelled,
    )
    .await;
    core.close().drain().await;
    if *cancelled.borrow() {
        return Err(Refusal::Cancelled.into());
    }
    match answer {
        Stage::Completed(answer) => answer,
        Stage::Cancelled => Err(Refusal::Cancelled.into()),
        Stage::Expired => Err(Refusal::TimedOut.into()),
    }
}

async fn attempts<O: Send + 'static>(
    instances: &Arc<Instances>,
    admission: &Admission,
    prepared: &mut Prepared<O>,
    cancelled: &mut watch::Receiver<bool>,
    budget: &Budget,
    instance: &Arc<Instance>,
    cause: Option<Position>,
) -> Result<O> {
    let mut attempt = 0;
    loop {
        if *cancelled.borrow() {
            return Err(Refusal::Cancelled.into());
        }
        let remaining = budget.remaining();
        if remaining.is_zero() {
            return Err(Refusal::TimedOut.into());
        }
        if !instance.attempt(attempt) {
            return Err(Refusal::Cancelled.into());
        }
        let core = ScopeCore::new(
            instances.clone(),
            admission.clone(),
            cause,
            Arc::downgrade(instance),
        );
        let answer = bounded(
            prepared.policy.attempt_timeout.min(remaining),
            prepared.body.run(Scope::over(&core)),
            cancelled,
        )
        .await;
        let mut children = core.close();
        if *cancelled.borrow() {
            children.drain().await;
            return Err(Refusal::Cancelled.into());
        }
        let (error, failure) = match answer {
            Stage::Completed(Ok(response)) => {
                children.drain().await;
                return if budget.remaining().is_zero() {
                    Err(Refusal::TimedOut.into())
                } else {
                    Ok(response)
                };
            }
            Stage::Completed(Err(error)) => {
                let failure = prepared.body.classify(&error);
                (error, failure)
            }
            Stage::Cancelled => {
                children.drain().await;
                return Err(Refusal::Cancelled.into());
            }
            Stage::Expired => (Refusal::TimedOut.into(), FailureKind::Ambiguous),
        };
        if prepared.body.transferred() {
            children.drain().await;
            return Err(error);
        }
        let delay = prepared.policy.retries.retry_after(
            attempt,
            failure,
            prepared.replay,
            prepared.id.value(),
        );
        let Some(delay) = delay else {
            children.drain().await;
            return Err(error);
        };
        instance.transition(Phase::RetryWaiting);
        futures_util::future::join(children.drain(), async {
            let _ = executor::timeout(
                budget.remaining(),
                interruptible(executor::sleep(delay), cancelled),
            )
            .await;
        })
        .await;
        if *cancelled.borrow() {
            return Err(Refusal::Cancelled.into());
        }
        if budget.remaining().is_zero() {
            return Err(Refusal::TimedOut.into());
        }
        // A Caller Can Transfer Input While The Retry Delay Is Pending.
        if prepared.body.transferred() {
            return Err(error);
        }
        // retry_after Rejects The Maximum Attempt Before This Increment.
        attempt += 1;
    }
}

async fn bounded<O>(
    timeout: Duration,
    future: Attempt<'_, O>,
    cancelled: &mut watch::Receiver<bool>,
) -> Stage<O> {
    match interruptible(executor::timeout(timeout, future), cancelled).await {
        None => Stage::Cancelled,
        Some(Err(_)) => Stage::Expired,
        Some(Ok(answer)) => Stage::Completed(answer),
    }
}

fn outcome<O>(answer: &Result<O>) -> Outcome {
    answer
        .as_ref()
        .err()
        .map_or(Outcome::Succeeded, failure_outcome)
}

fn failure_outcome(error: &Error) -> Outcome {
    match error {
        Error::OperationRefused(Refusal::Cancelled) => Outcome::Cancelled,
        Error::OperationRefused(Refusal::TimedOut) => Outcome::TimedOut,
        Error::OperationRefused(Refusal::Unavailable) => Outcome::Unavailable,
        _ => Outcome::Failed,
    }
}

/* === Tests === */
