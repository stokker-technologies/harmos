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
use std::sync::atomic::Ordering;
use std::task::Poll;
// Third Party Imports
use serde::Serialize;
use tokio::sync::watch;
// Local Imports
use super::manager::{Activation, Cleanup, ErasedResident, Failure};
use super::{Fault, Service, ServiceExit, ServicePolicy, ServiceStatus};
use crate::supervision::{Backoff, executor};
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
    fn under(policy: &ServicePolicy) -> Self {
        Self {
            credit: policy.credit,
            failures: 0,
            history: Vec::new(),
        }
    }

    fn refill(&mut self, policy: &ServicePolicy) {
        self.credit = policy.credit;
        self.failures = 0;
    }
}

/* === Public Functions === */
pub(super) async fn run<A: Application>(
    name: String,
    policy: ServicePolicy,
    resident: &mut dyn ErasedResident<A>,
    cleanup: &Cleanup<A>,
    services: Activation<A>,
    stopping: watch::Sender<bool>,
) -> Result<()> {
    let mut health = Health::under(&policy);
    let mut attempt_index = 0;

    loop {
        if !cleanup.lifecycle.initialize_attempt(attempt_index) {
            services.quiesce();
            cleanup.record(Ok(()));
            services.cleaning();
            if !cleanup.finalized.load(Ordering::Acquire) {
                finalize(resident).await?;
                cleanup.finalized.store(true, Ordering::Release);
            }
            services.set(&name, ServiceStatus::Stopped);
            return Ok(());
        }
        cleanup.finalized.store(false, Ordering::Release);
        services.set(&name, ServiceStatus::Starting);
        let (ready, mut readiness) = watch::channel(false);
        let context = Service {
            state: services.state.clone(),
            records: services.records.clone(),
            operations: services.operations.clone(),
            streams: services.streams.clone(),
            evidence: services.evidence.clone(),
            observers: services.observers.clone(),
            stopping: stopping.subscribe(),
            ready,
            #[cfg(feature = "plugin-sidecar")]
            provider: cleanup.lifecycle.clone(),
        };
        let mut attempt = Box::pin(crate::supervision::attempt(
            async { resident.attempt(context).await },
            stopping.subscribe(),
            policy.stop_timeout,
        ));

        let mut readiness_observed = false;
        let mut health_wait: Option<HealthWait> = None;
        let answer = loop {
            let event = next_event(
                attempt.as_mut(),
                &mut readiness,
                readiness_observed,
                &mut health_wait,
            )
            .await;
            match event {
                AttemptEvent::Completed(answer) => break answer,
                AttemptEvent::Ready => {
                    readiness_observed = true;
                    if !*stopping.borrow() {
                        services.set(
                            &name,
                            ServiceStatus::Running {
                                credit: health.credit,
                            },
                        );
                        if health.credit < policy.credit {
                            health_wait = Some(Box::pin(executor::sleep(policy.sustained_health)));
                        }
                    }
                }
                AttemptEvent::ReadinessClosed => readiness_observed = true,
                AttemptEvent::Healthy => {
                    health_wait = None;
                    if !*stopping.borrow() {
                        health.refill(&policy);
                        services.set(
                            &name,
                            ServiceStatus::Running {
                                credit: health.credit,
                            },
                        );
                    }
                }
            }
        }
        .map_err(|failure| match failure {
            crate::supervision::Failure::Panicked => ServiceExit::Panicked,
            crate::supervision::Failure::StopTimedOut => ServiceExit::StopTimedOut,
        });

        drop(attempt);
        let terminal = match &answer {
            Err(cause) => Some(Err(Arc::new(Error::TaskFailed {
                domain: "service attempt",
                source: Arc::new(cause.clone()),
            }))),
            Ok(Ok(())) => Some(Ok(())),
            Ok(Err(Fault::Application(error))) => Some(Err(Arc::new(Error::TaskFailed {
                domain: "service attempt",
                source: error.source.clone(),
            }))),
            Ok(Err(Fault::Environmental(error))) if health.credit == 0 && !*stopping.borrow() => {
                Some(Err(Arc::new(Error::TaskFailed {
                    domain: "service attempt",
                    source: error.source.clone(),
                })))
            }
            Ok(Err(Fault::Environmental(_))) if *stopping.borrow() => Some(Ok(())),
            Ok(Err(Fault::Environmental(_))) => None,
        };
        if let Some(terminal) = terminal {
            services.quiesce();
            cleanup.record(terminal);
        }
        services.cleaning();
        if let Err(error) = finalize(resident).await {
            if let Ok(Err(Fault::Environmental(failure))) = &answer {
                cleanup.record(Err(Arc::new(Error::TaskFailed {
                    domain: "service attempt",
                    source: failure.source.clone(),
                })));
            }
            services.quiesce();
            return Err(error);
        }
        cleanup.finalized.store(true, Ordering::Release);

        match answer {
            Ok(answer) => {
                if let Some(result) = settle(
                    &name,
                    answer,
                    &mut health,
                    &policy,
                    &services,
                    &stopping,
                    &cleanup.lifecycle,
                )
                .await
                {
                    return result;
                }
            }
            Err(cause) => {
                let source = Arc::new(cause.clone());
                return deactivate(&name, cause, health.history, source, &services).await;
            }
        }
        attempt_index = attempt_index
            .checked_add(1)
            .ok_or(Error::ResourceExhausted {
                resource: "service attempt identities",
            })?;
    }
}

pub(super) async fn finalize<A: Application>(resident: &mut dyn ErasedResident<A>) -> Result<()> {
    let mut future = std::pin::pin!(async { resident.finalize().await });
    std::future::poll_fn(|context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(poll) => poll.map(|result| {
                result.map_err(|failure| Error::TaskFailed {
                    domain: "service cleanup",
                    source: failure.source,
                })
            }),
            Err(_) => Poll::Ready(Err(Error::TaskFailed {
                domain: "service cleanup",
                source: Arc::new(ServiceExit::Panicked),
            })),
        }
    })
    .await
}

/* === Private Functions === */
async fn next_event<F: Future + ?Sized>(
    mut attempt: Pin<&mut F>,
    readiness: &mut watch::Receiver<bool>,
    readiness_observed: bool,
    health_wait: &mut Option<HealthWait>,
) -> AttemptEvent<F::Output> {
    let mut ready = std::pin::pin!(readiness.wait_for(|ready| *ready));
    std::future::poll_fn(|context| {
        if let Poll::Ready(answer) = attempt.as_mut().poll(context) {
            return Poll::Ready(AttemptEvent::Completed(answer));
        }
        if !readiness_observed && let Poll::Ready(answer) = ready.as_mut().poll(context) {
            return Poll::Ready(if answer.is_ok() {
                AttemptEvent::Ready
            } else {
                AttemptEvent::ReadinessClosed
            });
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
    name: &str,
    answer: std::result::Result<(), Fault<Failure>>,
    health: &mut Health,
    policy: &ServicePolicy,
    services: &Activation<A>,
    stopping: &watch::Sender<bool>,
    _provider: &Arc<crate::supervision::instance::Instance>,
) -> Option<Result<()>> {
    match answer {
        Ok(()) => {
            services.set(name, ServiceStatus::Stopped);
            Some(Ok(()))
        }
        Err(Fault::Application(error)) => Some(
            deactivate(
                name,
                ServiceExit::Application(error.message),
                std::mem::take(&mut health.history),
                error.source,
                services,
            )
            .await,
        ),
        Err(Fault::Environmental(error)) => {
            if *stopping.borrow() {
                services.set(name, ServiceStatus::Stopped);
                return Some(Ok(()));
            }
            health.history.push(error.message);
            if health.credit == 0 {
                return Some(
                    deactivate(
                        name,
                        ServiceExit::CreditExhausted,
                        std::mem::take(&mut health.history),
                        error.source,
                        services,
                    )
                    .await,
                );
            }
            health.credit -= 1;
            services.set(name, ServiceStatus::BackingOff);
            let mut context = Service {
                state: services.state.clone(),
                records: services.records.clone(),
                operations: services.operations.clone(),
                streams: services.streams.clone(),
                evidence: services.evidence.clone(),
                observers: services.observers.clone(),
                stopping: stopping.subscribe(),
                ready: watch::channel(false).0,
                #[cfg(feature = "plugin-sidecar")]
                provider: _provider.clone(),
            };
            if context
                .rest(Backoff::duration(
                    health.failures,
                    policy.backoff_base,
                    policy.backoff_cap,
                ))
                .await
            {
                services.set(name, ServiceStatus::Stopped);
                return Some(Ok(()));
            }
            health.failures += 1;
            None
        }
    }
}

async fn deactivate<A: Application>(
    name: &str,
    cause: ServiceExit,
    history: Vec<String>,
    source: Arc<dyn std::error::Error + Send + Sync>,
    services: &Activation<A>,
) -> Result<()> {
    let payload = serde_json::to_vec(&Deactivation {
        cause: describe(&cause),
        history: &history,
    })
    .expect("the fixed work fact shape serializes");
    let _ = services
        .records
        .publish_system(SystemFact {
            source: FactSource::Work(name.to_owned()),
            name: "deactivated".to_owned(),
            version: 1,
            payload,
        })
        .await;
    services.set(name, ServiceStatus::Inactive { cause, history });
    Err(Error::TaskFailed {
        domain: "service attempt",
        source,
    })
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
