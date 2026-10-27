/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The restart driver of one service instance. Each run is stopped and cleaned
// Up Before Replacement. Failures Spend Restart Credit; Sustained Health Refills
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
use crate::operations::resident::Report;
use crate::operations::{Manager, OperationPhase, RestartPolicy};
use crate::supervision::executor;
use crate::{Application, Error, Result, state::FactSource, state::SystemFact};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type HealthWait = Pin<Box<dyn Future<Output = ()> + Send>>;

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

/* === Traits === */

/* === Implementations === */

impl Health {
    fn under(restart: RestartPolicy) -> Self {
        Self { credit: restart.credit(), failures: 0, history: Vec::new() }
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

    loop {
        if !cleanup.lifecycle.initialize_attempt(attempt_index) {
            report.quiesce();
            cleanup.record(Ok(()));
            report.transition(OperationPhase::Stopping);
            if !cleanup.finalized.load(Ordering::Acquire) {
                super::runner::cleanup(resident).await?;
                cleanup.finalized.store(true, Ordering::Release);
            }
            report.health(ServiceStatus::Stopped);
            return Ok(());
        }
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
        let mut attempt =
            Box::pin(crate::supervision::attempt(async { resident.run(context).await }, stopping.subscribe(), policy.stop_timeout));

        let mut readiness_observed = false;
        let mut health_wait: Option<HealthWait> = None;
        let answer = loop {
            let event = next_event(attempt.as_mut(), &mut readiness, readiness_observed, &mut health_wait).await;
            match event {
                AttemptEvent::Completed(answer) => break answer,
                AttemptEvent::Ready => {
                    readiness_observed = true;
                    if !*stopping.borrow() {
                        report.health(ServiceStatus::Running { credit: health.credit });
                        if health.credit < policy.restart.credit()
                            && let RestartPolicy::OnFailure { sustained_health, .. } = policy.restart
                        {
                            health_wait = Some(Box::pin(executor::sleep(sustained_health)));
                        }
                    }
                }
                AttemptEvent::ReadinessClosed => readiness_observed = true,
                AttemptEvent::Healthy => {
                    health_wait = None;
                    if !*stopping.borrow() {
                        health.refill(policy.restart);
                        report.health(ServiceStatus::Running { credit: health.credit });
                    }
                }
            }
        }
        .map_err(|failure| match failure {
            crate::supervision::Failure::Panicked => ServiceExit::Panicked,
            crate::supervision::Failure::StopTimedOut => ServiceExit::StopTimedOut,
        });

        drop(attempt);
        #[cfg(feature = "plugin-sidecar")]
        let restart = if deactivation.load(Ordering::Acquire) { RestartPolicy::Never } else { policy.restart };
        #[cfg(not(feature = "plugin-sidecar"))]
        let restart = policy.restart;
        let terminal = match &answer {
            Err(cause) => Some(Err(Arc::new(Error::TaskFailed { domain: "service attempt", source: Arc::new(cause.clone()) }))),
            Ok(Ok(())) => Some(Ok(())),
            Ok(Err(error)) if (health.credit == 0 || matches!(restart, RestartPolicy::Never)) && !*stopping.borrow() => {
                Some(Err(Arc::new(Error::TaskFailed { domain: "service attempt", source: error.source.clone() })))
            }
            Ok(Err(_)) if *stopping.borrow() => Some(Ok(())),
            Ok(Err(_)) => None,
        };
        if let Some(terminal) = terminal {
            report.quiesce();
            cleanup.record(terminal);
        }
        report.transition(OperationPhase::Stopping);
        if let Err(error) = super::runner::cleanup(resident).await {
            if let Ok(Err(failure)) = &answer {
                cleanup.record(Err(Arc::new(Error::TaskFailed { domain: "service attempt", source: failure.source.clone() })));
            }
            report.quiesce();
            return Err(error);
        }
        cleanup.finalized.store(true, Ordering::Release);

        match answer {
            Ok(answer) => {
                if let Some(result) = settle(name, answer, &mut health, restart, report, &handles, &stopping).await {
                    return result;
                }
            }
            Err(cause) => {
                let source = Arc::new(cause.clone());
                return deactivate(name, cause, health.history, source, report, &handles).await;
            }
        }
        attempt_index = attempt_index.checked_add(1).ok_or(Error::ResourceExhausted { resource: "service attempt identities" })?;
    }
}

/* === Private Functions === */

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

async fn settle<A: Application>(
    name: &str, answer: std::result::Result<(), Failure>, health: &mut Health, restart: RestartPolicy, report: &Report,
    handles: &super::Handles<A>, stopping: &watch::Sender<bool>,
) -> Option<Result<()>> {
    match answer {
        Ok(()) => {
            report.health(ServiceStatus::Stopped);
            Some(Ok(()))
        }
        Err(error) => {
            if *stopping.borrow() {
                report.health(ServiceStatus::Stopped);
                return Some(Ok(()));
            }
            if matches!(restart, RestartPolicy::Never) {
                return Some(
                    deactivate(
                        name,
                        ServiceExit::Application(error.message),
                        std::mem::take(&mut health.history),
                        error.source,
                        report,
                        handles,
                    )
                    .await,
                );
            }
            health.history.push(error.message);
            let backoff = match restart {
                RestartPolicy::OnFailure { backoff, .. } if health.credit > 0 => backoff,
                _ => {
                    return Some(
                        deactivate(name, ServiceExit::CreditExhausted, std::mem::take(&mut health.history), error.source, report, handles)
                            .await,
                    );
                }
            };
            health.credit -= 1;
            report.health(ServiceStatus::BackingOff);
            let entropy = report.id().value();

            #[cfg(feature = "simulation")]
            let entropy = crate::simulation::executor::entropy_or(entropy);

            let delay = backoff.delay(health.failures, entropy).unwrap_or(Duration::ZERO);
            let mut waiting = stopping.subscribe();
            let interrupted = crate::supervision::interruptible(executor::sleep(delay), &mut waiting).await.is_none();
            if interrupted {
                report.health(ServiceStatus::Stopped);
                return Some(Ok(()));
            }
            health.failures += 1;
            None
        }
    }
}

async fn deactivate<A: Application>(
    name: &str, cause: ServiceExit, history: Vec<String>, source: Arc<dyn std::error::Error + Send + Sync>, report: &Report,
    handles: &super::Handles<A>,
) -> Result<()> {
    let payload =
        serde_json::to_vec(&Deactivation { cause: describe(&cause), history: &history }).expect("the fixed work fact shape serializes");
    let _ = handles
        .records
        .publish_system(SystemFact { source: FactSource::Work(name.to_owned()), name: "deactivated".to_owned(), version: 1, payload })
        .await;
    report.health(ServiceStatus::Inactive { cause, history });
    Err(Error::TaskFailed { domain: "service attempt", source })
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
