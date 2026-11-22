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
use tracing::Instrument;

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
use harmos_operations::clock::WindDown;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub(super) type Attempt<'a, O> = Pin<Box<dyn Future<Output = Result<O>> + Send + 'a>>;

/* === Structs === */

pub(crate) struct Prepared<O> {
    pub(super) body: Box<dyn Body<O>>,
    pub(super) policy: OperationPolicy,
    pub(super) operation: String,
    pub(super) timeout_source: &'static str,
    pub(super) replay: ReplaySafety,
    pub(super) id: OperationInstanceId,
    pub(super) refusal: Option<Error>,
    pub(super) middleware: Option<super::middleware::Session>,
    pub(super) activity: Option<crate::clock::Activity>,
}

pub(super) struct Execution {
    pub(super) admission: Admission,
    pub(super) ticket: super::lane::Ticket,
    pub(super) submitted: tokio::time::Instant,
    pub(super) cancelled: watch::Receiver<bool>,
    pub(super) instance: Arc<Instance>,
    pub(super) queue: QueuePermit,
}

struct Budget {
    submitted: tokio::time::Instant,
    operation: String,
    timeout_source: &'static str,
    timeout: Duration,
    progress: Vec<watch::Receiver<Option<tokio::time::Instant>>>,
}

/* === Enums === */

enum Stage<O> {
    Completed(Result<O>),
    Cancelled,
    Expired,
}

/* === Traits === */

pub(super) trait Body<O>: Send {
    fn scope_clock(&self) -> Option<crate::clock::Clock> {
        None
    }

    fn wind_down(&self) -> WindDown {
        WindDown::default()
    }

    /// Runs Once at Admission, Before Middleware Observes the Call.
    fn admit(&mut self) {}

    fn progress(&self) -> Vec<watch::Receiver<Option<tokio::time::Instant>>> {
        Vec::new()
    }

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

    fn retain(self: Box<Self>, outcome: Option<Error>, cause: Arc<Error>) -> (Box<dyn Finalizer>, Option<Error>, Arc<Error>);
}

/* === Implementations === */

impl Budget {
    fn timed_out(&self, attempt: usize) -> Refusal {
        Refusal::TimedOut {
            operation: self.operation.clone(),
            attempt,
            timeout: self.timeout,
            configured_by: self.timeout_source,
        }
    }

    fn cancelled(&self, attempt: usize) -> Refusal {
        Refusal::Cancelled {
            operation: self.operation.clone(),
            attempt,
        }
    }

    fn remaining(&self) -> Duration {
        let last = self.progress.iter().filter_map(|receiver| *receiver.borrow()).max().unwrap_or(self.submitted);
        self.timeout.saturating_sub(executor::now().saturating_duration_since(last.max(self.submitted)))
    }
}

impl<O> Prepared<O> {
    pub(super) fn retain_unavailable(mut self, control: &Instance, admission: Admission, clock: &crate::clock::Clock) {
        let outcome = self.refusal.take().or_else(|| Some(Refusal::Unavailable.into()));
        let cause = Arc::new(Error::Closing { domain: "preparation driver" });
        let (owner, _, diagnostic) = self.body.retain(outcome, cause);
        control.complete(Outcome::Unavailable);
        control.retain(
            Retained {
                owner,
                _permit: None,
                _admission: admission,
                _activity: clock.activity("retained invocation cleanup"),
            },
            diagnostic,
        );
    }
}

/* === Public Functions === */

#[tracing::instrument(target = "harmos::operations", name = "operations.execute", level = "debug", skip_all)]
pub(super) async fn execute<O: Send + 'static>(
    instances: Arc<Instances>, mut prepared: Prepared<O>, execution: Execution, cause: Option<Position>,
) -> Result<O> {
    let Execution {
        admission,
        ticket,
        submitted,
        mut cancelled,
        instance,
        queue,
    } = execution;
    let budget = Budget {
        submitted,
        operation: prepared.operation.clone(),
        timeout_source: prepared.timeout_source,
        timeout: prepared.policy.timeout,
        progress: prepared.body.progress(),
    };
    let mut middleware = prepared.middleware.take();
    let mut refusal = prepared.refusal.take();
    if refusal.is_none()
        && let Some(session) = &mut middleware
    {
        refusal = match bounded(budget.remaining(), Box::pin(session.enter()), &mut cancelled).await {
            Stage::Completed(answer) => answer.err(),
            Stage::Cancelled => Some(budget.cancelled(1).into()),
            Stage::Expired => Some(budget.timed_out(instance.status().attempt() + 1).into()),
        };
    }
    let was_refused = refusal.is_some();
    let mut permit = None;
    let answer = match refusal {
        Some(error) => Err(error),
        None => match crate::clock::waiting(acquire(&admission, ticket, &budget, &mut cancelled)).await {
            Err(error) => Err(error),
            Ok(acquired) => {
                permit = Some(acquired);
                queue.release();
                match initialize(&instances, &admission, prepared.body.as_mut(), &mut cancelled, &budget, &instance, cause).await {
                    Err(error) => Err(error),
                    Ok(()) => {
                        prepared.middleware = middleware.take();
                        let answer = attempts(&instances, &admission, &mut prepared, &mut cancelled, &budget, &instance, cause).await;
                        middleware = prepared.middleware.take();
                        answer
                    }
                }
            }
        },
    };

    instance.complete(outcome(&answer));
    instance.transition(prepared.body.cleanup_phase());
    if let Err(cleanup) =
        prepared.body.finalize().instrument(tracing::debug_span!(target: "harmos::operations", "operations.finalize")).await
    {
        let logical = match answer {
            Err(error) => Some(error),
            Ok(_) if !*cancelled.borrow() && budget.remaining().is_zero() => Some(budget.timed_out(instance.status().attempt() + 1).into()),
            Ok(_) => None,
        };
        instance.complete(logical.as_ref().map_or(Outcome::Succeeded, failure_outcome));
        let declared = logical.as_ref().map_or(FailureKind::Permanent, |error| prepared.body.classify(error));
        let (owner, failure, diagnostic) = prepared.body.retain(logical, Arc::new(cleanup));
        instance.retain(
            Retained {
                owner,
                _permit: permit,
                _admission: admission,
                _activity: instances.clock.activity("retained invocation cleanup"),
            },
            diagnostic,
        );
        let failure = failure.unwrap_or_else(|| Refusal::Unavailable.into());
        record_failure(&budget.operation, &failure, declared);
        if let Some(session) = &mut middleware {
            session.observe(super::middleware::Observation {
                outcome: failure_outcome(&failure),
                settled: false,
            });
        }
        return Err(failure);
    }
    let declared = answer.as_ref().err().map(|error| prepared.body.classify(error));
    drop(prepared.body);
    let answer = if was_refused {
        answer
    } else if !*cancelled.borrow() && budget.remaining().is_zero() {
        Err(budget.timed_out(instance.status().attempt() + 1).into())
    } else {
        answer
    };
    drop(permit);
    drop(admission);
    instance.complete(outcome(&answer));
    instance.finished();
    if let Err(error) = &answer {
        record_failure(&budget.operation, error, declared.unwrap_or(FailureKind::Permanent));
    }
    if let Some(session) = &mut middleware {
        session.observe(super::middleware::Observation { outcome: outcome(&answer), settled: true });
    }
    answer
}

/* === Private Functions === */

#[tracing::instrument(target = "harmos::operations", name = "operations.acquire", level = "debug", skip_all)]
async fn acquire(
    admission: &Admission, ticket: super::lane::Ticket, budget: &Budget, cancelled: &mut watch::Receiver<bool>,
) -> Result<OwnedSemaphorePermit> {
    if admission.closed() {
        return Err(Refusal::Unavailable.into());
    }
    if *cancelled.borrow() {
        return Err(budget.cancelled(1).into());
    }
    if budget.remaining().is_zero() {
        return Err(budget.timed_out(1).into());
    }
    let waiting = interruptible(ticket, cancelled);
    match executor::timeout(budget.remaining(), waiting).await {
        Err(_) => Err(budget.timed_out(1).into()),
        Ok(None) if admission.closed() => Err(Refusal::Unavailable.into()),
        Ok(None) => Err(budget.cancelled(1).into()),
        Ok(Some(Err(_))) => Err(Refusal::Unavailable.into()),
        Ok(Some(Ok(_))) if admission.closed() => Err(Refusal::Unavailable.into()),
        Ok(Some(Ok(permit))) => Ok(permit),
    }
}

#[tracing::instrument(target = "harmos::operations", name = "operations.initialize", level = "debug", skip_all)]
async fn initialize<O: Send + 'static>(
    instances: &Arc<Instances>, admission: &Admission, body: &mut dyn Body<O>, cancelled: &mut watch::Receiver<bool>, budget: &Budget,
    instance: &Arc<Instance>, cause: Option<Position>,
) -> Result<()> {
    if !instance.initialize_attempt(0) {
        return Err(budget.cancelled(1).into());
    }
    let core = ScopeCore::new(instances.clone(), admission.clone(), cause, Arc::downgrade(instance), body.scope_clock(), body.wind_down());
    let answer = bounded(budget.remaining(), body.initialize(Scope::over(&core)), cancelled).await;
    crate::clock::waiting(core.close().drain()).await;
    if *cancelled.borrow() {
        return Err(budget.cancelled(1).into());
    }
    match answer {
        Stage::Completed(answer) => answer,
        Stage::Cancelled => Err(budget.cancelled(1).into()),
        Stage::Expired => Err(budget.timed_out(1).into()),
    }
}

#[tracing::instrument(target = "harmos::operations", name = "operations.attempts", level = "debug", skip_all)]
async fn attempts<O: Send + 'static>(
    instances: &Arc<Instances>, admission: &Admission, prepared: &mut Prepared<O>, cancelled: &mut watch::Receiver<bool>, budget: &Budget,
    instance: &Arc<Instance>, cause: Option<Position>,
) -> Result<O> {
    let mut attempt = 0;
    loop {
        if *cancelled.borrow() {
            return Err(budget.cancelled(attempt + 1).into());
        }
        let remaining = budget.remaining();
        if remaining.is_zero() {
            return Err(budget.timed_out(attempt + 1).into());
        }
        if !instance.attempt(attempt) {
            return Err(budget.cancelled(attempt + 1).into());
        }
        let core = ScopeCore::new(
            instances.clone(),
            admission.clone(),
            cause,
            Arc::downgrade(instance),
            prepared.body.scope_clock(),
            prepared.body.wind_down(),
        );
        let started_at = prepared.middleware.as_ref().map(super::middleware::Session::now);
        let answer = cooperative(
            prepared.policy.attempt_timeout.min(remaining),
            budget,
            prepared.policy.cancellation_timeout,
            core.cancellation(),
            Box::pin(
                prepared
                    .body
                    .run(Scope::over(&core))
                    .instrument(tracing::debug_span!(target: "harmos::operations", "operations.attempt", attempt)),
            ),
            cancelled,
        )
        .await;
        let finished_at = prepared.middleware.as_ref().map(super::middleware::Session::now);
        let mut children = core.close();
        if let Some(middleware) = &prepared.middleware {
            let outcome = match &answer {
                Stage::Completed(Ok(_)) => Outcome::Succeeded,
                Stage::Completed(Err(Error::OperationRefused(Refusal::Cancelled { .. }))) | Stage::Cancelled => Outcome::Cancelled,
                Stage::Completed(Err(Error::OperationRefused(Refusal::TimedOut { .. }))) | Stage::Expired => Outcome::TimedOut,
                Stage::Completed(Err(_)) => Outcome::Failed,
            };
            // Record Each Completed Attempt Before Its Result Or Retry Is Exposed.
            let started_at = started_at.expect("middleware attempt start");
            let elapsed = finished_at.expect("middleware attempt end").instant.saturating_duration_since(started_at.instant);
            if let Err(error) = middleware.attempt(super::middleware::AttemptObservation { attempt, outcome, started_at, elapsed }).await {
                crate::clock::waiting(children.drain()).await;
                return Err(error);
            }
        }
        if *cancelled.borrow() {
            crate::clock::waiting(children.drain()).await;
            return match answer {
                Stage::Completed(answer) => answer,
                Stage::Cancelled | Stage::Expired => Err(budget.cancelled(attempt + 1).into()),
            };
        }
        let (error, failure) = match answer {
            Stage::Completed(Ok(response)) => {
                crate::clock::waiting(children.drain()).await;
                return if !*cancelled.borrow() && budget.remaining().is_zero() {
                    Err(budget.timed_out(attempt + 1).into())
                } else {
                    Ok(response)
                };
            }
            Stage::Completed(Err(error)) => {
                let failure = prepared.body.classify(&error);
                (error, failure)
            }
            Stage::Cancelled => {
                crate::clock::waiting(children.drain()).await;
                return Err(budget.cancelled(attempt + 1).into());
            }
            Stage::Expired => {
                let refusal = if budget.remaining().is_zero() || !budget.progress.is_empty() {
                    budget.timed_out(attempt + 1)
                } else {
                    Refusal::TimedOut {
                        operation: prepared.operation.clone(),
                        attempt: attempt + 1,
                        timeout: prepared.policy.attempt_timeout,
                        configured_by: "operation policy.attempt_timeout",
                    }
                };
                (refusal.into(), FailureKind::Ambiguous)
            }
        };
        if prepared.body.transferred() {
            crate::clock::waiting(children.drain()).await;
            return Err(error);
        }
        let delay = prepared.policy.retries.retry_after(attempt, failure, prepared.replay, prepared.id.value());
        let Some(delay) = delay else {
            crate::clock::waiting(children.drain()).await;
            return Err(error);
        };
        instance.transition(Phase::RetryWaiting);
        futures_util::future::join(children.drain(), async {
            let _ = executor::timeout(budget.remaining(), interruptible(executor::sleep(delay), cancelled))
                .instrument(tracing::debug_span!(target: "harmos::operations", "operations.retry_wait"))
                .await;
        })
        .await;
        if *cancelled.borrow() {
            return Err(budget.cancelled(attempt + 1).into());
        }
        if budget.remaining().is_zero() {
            return Err(budget.timed_out(attempt + 1).into());
        }
        // A Caller Can Transfer Input While The Retry Delay Is Pending.
        if prepared.body.transferred() {
            return Err(error);
        }
        // retry_after Rejects The Maximum Attempt Before This Increment.
        attempt += 1;
    }
}

async fn bounded<O>(timeout: Duration, future: Attempt<'_, O>, cancelled: &mut watch::Receiver<bool>) -> Stage<O> {
    match interruptible(executor::timeout(timeout, future), cancelled).await {
        None => Stage::Cancelled,
        Some(Err(_)) => Stage::Expired,
        Some(Ok(answer)) => Stage::Completed(answer),
    }
}

async fn cooperative<O>(
    timeout: Duration, budget: &Budget, grace: Duration, signal: &harmos_operations::execution::operations::Cancellation,
    future: Attempt<'_, O>, cancelled: &mut watch::Receiver<bool>,
) -> Stage<O> {
    let running = async {
        if budget.progress.is_empty() {
            return executor::timeout(timeout, future).await;
        }
        tokio::pin!(future);
        loop {
            let changed = futures_util::future::select_all(budget.progress.iter().map(|receiver| {
                let mut receiver = receiver.clone();
                let observed = *receiver.borrow_and_update();
                Box::pin(async move {
                    if receiver.wait_for(|moved| *moved != observed).await.is_err() {
                        std::future::pending::<()>().await;
                    }
                })
            }));
            let remaining = budget.remaining();
            tokio::select! {
                biased;
                _ = changed => {}
                answer = executor::timeout(remaining, &mut future) => return answer,
            }
        }
    };
    tokio::pin!(running);
    if let Some(answer) = interruptible(&mut running, cancelled).await {
        return match answer {
            Ok(answer) => Stage::Completed(answer),
            Err(_) => Stage::Expired,
        };
    }
    signal.request();
    if grace.is_zero() {
        return Stage::Cancelled;
    }
    match executor::timeout(grace, &mut running).await {
        Ok(Ok(answer)) => Stage::Completed(answer),
        _ => Stage::Cancelled,
    }
}

fn outcome<O>(answer: &Result<O>) -> Outcome {
    answer.as_ref().err().map_or(Outcome::Succeeded, failure_outcome)
}

/// Write the One Record of a Request That Completed With a Failure; Payload Contents Never Enter It.
///
/// Every request, nested ones included, completes through `execute` exactly once, so no other layer logs the same failure.
/// Sample rows travel inside a window's answer, never as requests, so a high-rate stream adds no records per sample.
/// A cancellation is the caller's own decision and writes none.
fn record_failure(operation: &str, error: &Error, declared: FailureKind) {
    let Some((code, caller_correctable)) = classify(error, declared) else { return };
    if caller_correctable {
        tracing::warn!(target: "harmos::operations", %operation, code, message = %error, "operation request failed");
    } else {
        tracing::error!(target: "harmos::operations", %operation, code, message = %error, "operation request failed");
    }
}

/// The Failure's Code and Whether the Caller Can Correct It; None for a Cancellation.
///
/// A structured failure names its own kind; otherwise the operation's declared classification of the error does.
fn classify(error: &Error, declared: FailureKind) -> Option<(&'static str, bool)> {
    match error {
        Error::OperationRefused(Refusal::Cancelled { .. }) => None,
        Error::OperationRefused(Refusal::TimedOut { .. }) => Some(("timed_out", true)),
        Error::OperationRefused(Refusal::Unavailable) => Some(("unavailable", true)),
        Error::NotRegistered { .. } | Error::OperationUnauthorized { .. } | Error::PermissionDenied => Some(("refused", true)),
        Error::InvalidInput { .. } => Some(("invalid", true)),
        error => match error.operation_failure().map_or(declared, |failure| failure.code()) {
            FailureKind::Cancelled => None,
            FailureKind::Transient => Some(("transient", true)),
            FailureKind::Invalid => Some(("invalid", true)),
            FailureKind::Unauthorized => Some(("unauthorized", true)),
            FailureKind::Ambiguous => Some(("ambiguous", false)),
            FailureKind::Permanent => Some(("permanent", false)),
        },
    }
}

fn failure_outcome(error: &Error) -> Outcome {
    match error {
        Error::OperationRefused(Refusal::Cancelled { .. }) => Outcome::Cancelled,
        Error::OperationRefused(Refusal::TimedOut { .. }) => Outcome::TimedOut,
        Error::OperationRefused(Refusal::Unavailable) => Outcome::Unavailable,
        _ => Outcome::Failed,
    }
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn cancel_final_response_grace_is_bounded_by_attempt() {
        let (cancel, mut cancelled) = watch::channel(false);
        let signal = harmos_operations::execution::operations::Cancellation::default();
        let entered = Arc::new(tokio::sync::Notify::new());
        let started = entered.clone();
        let work: Attempt<'_, ()> = Box::pin(async move {
            started.notify_one();
            std::future::pending().await
        });
        let budget = Budget {
            submitted: executor::now(),
            operation: "test.cancel".into(),
            timeout_source: "operation policy.timeout",
            timeout: Duration::from_millis(10),
            progress: Vec::new(),
        };
        let running = cooperative(Duration::from_millis(10), &budget, Duration::from_secs(1), &signal, work, &mut cancelled);
        tokio::pin!(running);
        tokio::select! {
            _ = &mut running => panic!("attempt must remain pending"),
            _ = entered.notified() => {}
        }
        cancel.send_replace(true);
        let before = executor::now();
        assert!(matches!(running.await, Stage::Cancelled));
        assert_eq!(executor::now() - before, Duration::from_millis(10));
        assert!(signal.is_requested());
    }
}
