/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Restart Driver Of One Service Instance. Each Run Is Stopped And Cleaned Up
// Before Replacement. Failures Spend Restart Credit; Sustained Health Refills
// Credit. Panics, Exhausted Credit And Missed Stop Deadlines Deactivate The Service.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use serde::Serialize;
use tokio::sync::watch;
// Local Imports
use super::runner::{Cleanup, ErasedResident, Failure};
use super::{ServiceDefinition, ServiceExit, ServiceStatus};
use crate::operations::resident::{Outcome, Report};
use crate::operations::{Manager, OperationPhase, RestartPolicy};
use crate::supervision::executor;
use crate::{Application, Error, Result, state::FactSource, state::SystemFact};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type HealthWait = Pin<Box<dyn Future<Output = ()> + Send>>;
type Answer = std::result::Result<std::result::Result<(), Failure>, ServiceExit>;

/* === Structs === */

struct Health {
    credit: usize,
    failures: usize,
    history: Vec<String>,
}

#[derive(Serialize)]
struct Deactivation<'a> {
    cause: String,
    history: &'a [String],
}

/* === Enums === */

enum AttemptEvent<T> {
    Completed(T),
    Ready,
    ReadinessClosed,
    Healthy,
}

enum Settled {
    Restart,
    Stopped,
}

/* === Traits === */

/* === Implementations === */

impl Health {
    fn under(restart: RestartPolicy) -> Self {
        Self {
            credit: restart.credit(),
            failures: 0,
            history: Vec::new(),
        }
    }

    fn refill(&mut self, restart: RestartPolicy) {
        self.credit = restart.credit();
        self.failures = 0;
    }
}

/* === Public Functions === */

pub(super) async fn run<A: Application>(
    definition: &ServiceDefinition, resident: &mut dyn ErasedResident<A>, cleanup: &Cleanup<A>, report: &Report, operations: &Manager,
    stopping: watch::Sender<bool>,
) -> Result<()> {
    let name = definition.id.as_str();
    let policy = definition.policy;
    let handles = operations.handles::<A>();
    let mut health = Health::under(policy.restart);
    let mut attempt_index = 0;

    while cleanup.lifecycle.initialize_attempt(attempt_index) {
        // Supervise One Attempt
        cleanup.finalized.store(false, Ordering::Release);
        report.health(ServiceStatus::Starting);
        let (ready, mut readiness) = watch::channel(false);
        let context = handles.context(
            operations.clone(),
            stopping.subscribe(),
            ready,
            #[cfg(feature = "plugin-sidecar")]
            Arc::clone(&cleanup.lifecycle),
        );
        #[cfg(feature = "plugin-sidecar")]
        let deactivation = context.deactivation.clone();
        let attempt = crate::supervision::attempt(async { resident.run(context).await }, stopping.subscribe(), policy.stop_timeout);
        let answer = supervise(attempt, &mut readiness, &mut health, policy.restart, report, &stopping).await;

        // Clean Up Before Settling
        #[cfg(feature = "plugin-sidecar")]
        let restart = match deactivation.load(Ordering::Acquire) {
            true => RestartPolicy::Never,
            false => policy.restart,
        };
        #[cfg(not(feature = "plugin-sidecar"))]
        let restart = policy.restart;
        let exhausted = health.credit == 0 || matches!(restart, RestartPolicy::Never);
        conclude(&answer, exhausted, resident, cleanup, report, &stopping).await?;

        let Settled::Restart = settle(name, answer, &mut health, restart, report, &handles, &stopping).await? else {
            return Ok(());
        };
        attempt_index = attempt_index.checked_add(1).ok_or(Error::ResourceExhausted { resource: "service attempt identities" })?;
    }

    stop(resident, cleanup, report).await
}

/* === Private Functions === */

async fn supervise<F>(
    attempt: F, readiness: &mut watch::Receiver<bool>, health: &mut Health, restart: RestartPolicy, report: &Report,
    stopping: &watch::Sender<bool>,
) -> Answer
where
    F: Future<Output = std::result::Result<std::result::Result<(), Failure>, crate::supervision::Failure>>,
{
    let mut attempt = Box::pin(attempt);
    let mut readiness_observed = false;
    let mut health_wait: Option<HealthWait> = None;

    let answer = loop {
        let event = next_event(attempt.as_mut(), readiness, readiness_observed, &mut health_wait).await;
        readiness_observed |= matches!(event, AttemptEvent::Ready | AttemptEvent::ReadinessClosed);
        match event {
            AttemptEvent::Completed(answer) => break answer,
            AttemptEvent::Ready => health_wait = running(health, restart, report, stopping),
            AttemptEvent::ReadinessClosed => {}
            AttemptEvent::Healthy => sustained(&mut health_wait, health, restart, report, stopping),
        }
    };
    answer.map_err(|failure| match failure {
        crate::supervision::Failure::Panicked => ServiceExit::Panicked,
        crate::supervision::Failure::StopTimedOut => ServiceExit::StopTimedOut,
    })
}

async fn next_event<F: Future + ?Sized>(
    mut attempt: Pin<&mut F>, readiness: &mut watch::Receiver<bool>, readiness_observed: bool, health_wait: &mut Option<HealthWait>,
) -> AttemptEvent<F::Output> {
    let mut ready = std::pin::pin!(readiness.wait_for(|ready| *ready));
    std::future::poll_fn(|context| {
        if let Poll::Ready(answer) = attempt.as_mut().poll(context) {
            return Poll::Ready(AttemptEvent::Completed(answer));
        }
        if !readiness_observed && let Poll::Ready(answer) = ready.as_mut().poll(context) {
            return Poll::Ready(if answer.is_ok() { AttemptEvent::Ready } else { AttemptEvent::ReadinessClosed });
        }
        if let Some(wait) = health_wait
            && wait.as_mut().poll(context).is_ready()
        {
            return Poll::Ready(AttemptEvent::Healthy);
        }
        Poll::Pending
    })
    .await
}

// Ready With Spent Credit Starts The Sustained Health Wait
fn running(health: &Health, restart: RestartPolicy, report: &Report, stopping: &watch::Sender<bool>) -> Option<HealthWait> {
    if *stopping.borrow() {
        return None;
    }
    report.health(ServiceStatus::Running { credit: health.credit });
    if health.credit < restart.credit()
        && let RestartPolicy::OnFailure { sustained_health, .. } = restart
    {
        let wait: HealthWait = Box::pin(executor::sleep(sustained_health));
        return Some(wait);
    }
    None
}

fn sustained(
    health_wait: &mut Option<HealthWait>, health: &mut Health, restart: RestartPolicy, report: &Report, stopping: &watch::Sender<bool>,
) {
    *health_wait = None;
    if *stopping.borrow() {
        return;
    }
    health.refill(restart);
    report.health(ServiceStatus::Running { credit: health.credit });
}

async fn conclude<A: Application>(
    answer: &Answer, exhausted: bool, resident: &mut dyn ErasedResident<A>, cleanup: &Cleanup<A>, report: &Report,
    stopping: &watch::Sender<bool>,
) -> Result<()> {
    if let Some(terminal) = terminal(answer, exhausted, stopping) {
        report.quiesce();
        cleanup.record(terminal);
    }
    report.transition(OperationPhase::Stopping);

    let Err(error) = super::runner::cleanup(resident).await else {
        cleanup.finalized.store(true, Ordering::Release);
        return Ok(());
    };
    if let Ok(Err(failure)) = answer {
        let failed = attempt_failed(failure.source.clone());
        cleanup.record(Err(failed));
    }
    report.quiesce();
    Err(error)
}

fn terminal(answer: &Answer, exhausted: bool, stopping: &watch::Sender<bool>) -> Option<Outcome> {
    let source: Arc<dyn std::error::Error + Send + Sync> = match answer {
        Err(cause) => Arc::new(cause.clone()),
        Ok(Ok(())) => return Some(Ok(())),
        Ok(Err(error)) if exhausted && !*stopping.borrow() => error.source.clone(),
        Ok(Err(_)) if *stopping.borrow() => return Some(Ok(())),
        Ok(Err(_)) => return None,
    };
    let failed = attempt_failed(source);
    Some(Err(failed))
}

fn attempt_failed(source: Arc<dyn std::error::Error + Send + Sync>) -> Arc<Error> {
    Arc::new(Error::TaskFailed { domain: "service attempt", source })
}

async fn settle<A: Application>(
    name: &str, answer: Answer, health: &mut Health, restart: RestartPolicy, report: &Report, handles: &super::Handles<A>,
    stopping: &watch::Sender<bool>,
) -> Result<Settled> {
    let error = match answer {
        Ok(Ok(())) => {
            report.health(ServiceStatus::Stopped);
            return Ok(Settled::Stopped);
        }
        Ok(Err(error)) => error,
        Err(cause) => {
            let source = Arc::new(cause.clone());
            let history = std::mem::take(&mut health.history);
            return Err(deactivate(name, cause, history, source, report, handles).await);
        }
    };
    if *stopping.borrow() {
        report.health(ServiceStatus::Stopped);
        return Ok(Settled::Stopped);
    }

    // Spend Credit Or Deactivate
    if matches!(restart, RestartPolicy::Never) {
        let cause = ServiceExit::Application(error.message);
        let history = std::mem::take(&mut health.history);
        return Err(deactivate(name, cause, history, error.source, report, handles).await);
    }
    health.history.push(error.message);
    let backoff = match restart {
        RestartPolicy::OnFailure { backoff, .. } if health.credit > 0 => backoff,
        _ => {
            let history = std::mem::take(&mut health.history);
            return Err(deactivate(name, ServiceExit::CreditExhausted, history, error.source, report, handles).await);
        }
    };
    health.credit -= 1;
    report.health(ServiceStatus::BackingOff);
    let entropy = report.id().value();

    #[cfg(feature = "simulation")]
    let entropy = crate::simulation::executor::entropy_or(entropy);

    // Back Off Unless Stopped
    let delay = backoff.delay(health.failures, entropy).unwrap_or(Duration::ZERO);
    let mut waiting = stopping.subscribe();
    let interrupted = crate::supervision::interruptible(executor::sleep(delay), &mut waiting).await.is_none();
    if interrupted {
        report.health(ServiceStatus::Stopped);
        return Ok(Settled::Stopped);
    }
    health.failures += 1;
    Ok(Settled::Restart)
}

async fn deactivate<A: Application>(
    name: &str, cause: ServiceExit, history: Vec<String>, source: Arc<dyn std::error::Error + Send + Sync>, report: &Report,
    handles: &super::Handles<A>,
) -> Error {
    let payload = serde_json::to_vec(&Deactivation {
        cause: describe(&cause),
        history: &history,
    })
    .expect("the fixed work fact shape serializes");
    let fact = SystemFact {
        source: FactSource::Work(name.to_owned()),
        name: "deactivated".to_owned(),
        version: 1,
        payload,
    };
    let _ = handles.records.publish_system(fact).await;
    report.health(ServiceStatus::Inactive { cause, history });
    Error::TaskFailed { domain: "service attempt", source }
}

async fn stop<A: Application>(resident: &mut dyn ErasedResident<A>, cleanup: &Cleanup<A>, report: &Report) -> Result<()> {
    report.quiesce();
    cleanup.record(Ok(()));
    report.transition(OperationPhase::Stopping);
    if !cleanup.finalized.load(Ordering::Acquire) {
        super::runner::cleanup(resident).await?;
        cleanup.finalized.store(true, Ordering::Release);
    }
    report.health(ServiceStatus::Stopped);
    Ok(())
}

fn describe(cause: &ServiceExit) -> String {
    match cause {
        ServiceExit::Application(error) => format!("application: {error}"),
        ServiceExit::Panicked => "panic".to_owned(),
        ServiceExit::CreditExhausted => "restart credit exhausted".to_owned(),
        ServiceExit::StopTimedOut => "stop deadline exceeded".to_owned(),
    }
}

/* === Tests === */
