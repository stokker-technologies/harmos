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
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{assembly, service_status, support::Bank};
use harmos_runtime::{
    Runtime, services::Resident, services::ServiceExit, services::ServiceStatus, state::FactSource, state::Position, state::SystemFact,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct MutableAdapter {
    attempts: usize,
    exits: tokio::sync::mpsc::UnboundedReceiver<AdapterExit>,
}

/* === Enums === */

enum AdapterExit {
    Environmental,
    Application,
}

/* === Traits === */

/* === Implementations === */

impl Resident<Bank> for MutableAdapter {
    type Error = std::io::Error;

    async fn run(&mut self, mut context: harmos_runtime::services::ServiceContext<Bank>) -> Result<(), Self::Error> {
        self.attempts += 1;
        let attempt = self.attempts;
        context.ready();
        match context.until_stopping(self.exits.recv()).await {
            None => Ok(()),
            Some(Some(AdapterExit::Environmental)) => Err(std::io::Error::other(format!("reset on attempt {attempt}"))),
            Some(Some(AdapterExit::Application)) => Err(std::io::Error::other(format!("failed on attempt {attempt}"))),
            Some(None) => Err(std::io::Error::other("exit channel closed")),
        }
    }
    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn work_facts(runtime: &Runtime<Bank>) -> Vec<SystemFact> {
    runtime
        .state
        .query::<SystemFact>(Position::ORIGIN)
        .await
        .map(|entry| entry.expect("service evidence is readable"))
        .map(|entry| entry.value)
        .filter(|fact| matches!(fact.source, FactSource::Work(_)))
        .collect()
}

/* === Tests === */

#[tokio::test]
async fn never_restart_preserves_failure() {
    for panics in [false, true] {
        let runtime = assembly()
            .service_with(
                "analysis",
                harmos_runtime::ServicePolicy { restart: harmos_runtime::RestartPolicy::Never, ..harmos_runtime::services::POLICY },
                move |_context| async move {
                    assert!(!panics, "analysis invariant panicked");
                    Err::<(), _>(std::io::Error::other("bad invariant"))
                },
            )
            .runtime()
            .await
            .expect("the service declaration is valid");
        let status =
            service_status(runtime.operations.watch_residents(), "analysis", |status| matches!(status, ServiceStatus::Inactive { .. }))
                .await;
        let ServiceStatus::Inactive { cause, history } = status else {
            unreachable!("the status matched Inactive");
        };
        assert_eq!(cause, if panics { ServiceExit::Panicked } else { ServiceExit::Application("bad invariant".into()) });
        assert!(history.is_empty());
        let facts = work_facts(&runtime).await;
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].identify().0.as_str(), "harmos:work/analysis/deactivated");
        assert!(matches!(runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
    }
}

#[tokio::test(start_paused = true)]
async fn credit_exhaustion_history() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let service_attempts = Arc::clone(&attempts);
    let runtime = assembly()
        .service("adapter", move |_context| {
            let attempts = Arc::clone(&service_attempts);
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(std::io::Error::other("still disconnected"))
            }
        })
        .runtime()
        .await
        .expect("the service declaration is valid");
    let status =
        service_status(runtime.operations.watch_residents(), "adapter", |status| matches!(status, ServiceStatus::Inactive { .. })).await;

    assert!(matches!(
        status,
        ServiceStatus::Inactive {
            cause: ServiceExit::CreditExhausted,
            ref history,
        } if history == &["still disconnected"; 4]
    ));
    assert_eq!(attempts.load(Ordering::SeqCst), 4);
    assert_eq!(work_facts(&runtime).await.len(), 1);
    assert!(matches!(runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
}

#[tokio::test(start_paused = true)]
async fn credit_survive_restarts() {
    let (exits, receiver) = tokio::sync::mpsc::unbounded_channel();
    let policy = harmos_runtime::ServicePolicy {
        restart: harmos_runtime::operations::RestartPolicy::OnFailure {
            credit: 1,
            sustained_health: Duration::from_millis(20),
            backoff: harmos_runtime::operations::Backoff::Exponential {
                initial: Duration::from_millis(1),
                maximum: Duration::from_millis(1),
                jitter: harmos_runtime::operations::Jitter::Deterministic,
            },
        },
        ..harmos_runtime::services::POLICY
    };
    let runtime = assembly()
        .service_with("adapter", policy, MutableAdapter { attempts: 0, exits: receiver })
        .runtime()
        .await
        .expect("the mutable resident declaration is valid");
    let mut statuses = runtime.operations.watch_residents();
    service_status(statuses.clone(), "adapter", |status| *status == ServiceStatus::Running { credit: 1 }).await;

    let key = harmos_runtime::operations::resident::Key::root("adapter");
    let instance_id = runtime.operations.resident(&key).unwrap().instance;

    exits.send(AdapterExit::Environmental).expect("the first attempt is live");
    service_status(statuses.clone(), "adapter", |status| *status == ServiceStatus::Running { credit: 0 }).await;
    assert_eq!(runtime.operations.resident(&key).unwrap().instance, instance_id);
    statuses
        .wait_for(|all| {
            all.get(&harmos_runtime::operations::resident::Key::root("adapter")).and_then(|item| item.health.as_ref())
                == Some(&ServiceStatus::Running { credit: 1 })
        })
        .await
        .expect("sustained health refills the visible credit");

    exits.send(AdapterExit::Application).expect("the second attempt is live");
    service_status(statuses.clone(), "adapter", |status| *status == ServiceStatus::Running { credit: 0 }).await;
    assert_eq!(runtime.operations.resident(&key).unwrap().instance, instance_id);
    exits.send(AdapterExit::Application).expect("the third attempt is live");
    let status = service_status(statuses, "adapter", |status| matches!(status, ServiceStatus::Inactive { .. })).await;
    assert!(matches!(status, ServiceStatus::Inactive { cause: ServiceExit::CreditExhausted, ref history }
        if history == &["reset on attempt 1", "failed on attempt 2", "failed on attempt 3"]));
    assert!(matches!(runtime.stop().await, Err(harmos_runtime::Error::TaskFailed { domain: "service", .. })));
}
