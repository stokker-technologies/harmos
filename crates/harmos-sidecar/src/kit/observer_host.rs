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
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{ObserverContext, PublicationError, Supervisor};
use harmos_domain::{StreamSpec, inventory::publications};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct CadenceReporter(Arc<dyn Fn(&crate::CadenceReport) + Send + Sync>);

/* === Enums === */

/* === Traits === */

pub trait ObserverHost: Send + Sync + Sized + 'static {
    fn bind(resources: Arc<harmos_sdk::ExecutionContext>, supervisor: Supervisor) -> Self;

    fn report_cadence(&self, _report: &crate::CadenceReport) {}
}

pub trait ObserverPublisher<V: Send>: Send + Sync + Sized {
    type Error: std::error::Error + From<PublicationError> + Send + Sync + 'static;
    const STREAM: Option<publications::Entry> = None;

    fn publish(&self, context: &ObserverContext<'_, Self>, value: V) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

pub trait ObserverStreamPublisher<S: StreamSpec>: Send + Sync + Sized
where
    S::Row: Sync,
{
    type Error: std::error::Error + From<PublicationError> + Send + Sync + 'static;

    fn publish_on(&self, context: &ObserverContext<'_, Self>, row: &S::Row) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/* === Implementations === */

impl CadenceReporter {
    pub fn new(report: impl Fn(&crate::CadenceReport) + Send + Sync + 'static) -> Self {
        Self(Arc::new(report))
    }

    pub fn for_host<H: ObserverHost>(host: H) -> Self {
        Self::new(move |report| host.report_cadence(report))
    }

    pub fn report(&self, report: &crate::CadenceReport) {
        (self.0)(report);
    }
}

impl ObserverHost for Supervisor {
    fn bind(_: Arc<harmos_sdk::ExecutionContext>, supervisor: Supervisor) -> Self {
        supervisor
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct Host(Arc<std::sync::Mutex<Vec<crate::CadenceReport>>>);

    impl ObserverHost for Host {
        fn bind(resources: Arc<harmos_sdk::ExecutionContext>, _: Supervisor) -> Self {
            resources.resource::<Self>().unwrap().as_ref().clone()
        }

        fn report_cadence(&self, report: &crate::CadenceReport) {
            self.0.lock().unwrap().push(report.clone());
        }
    }

    #[tokio::test]
    async fn queued_wire_reports_reach_the_host_hook_without_blocking_ticks() {
        let (sending, mut received) = tokio::sync::mpsc::channel(1);
        let supervisor = Supervisor::new(sending, || None);
        let host = Host::default();
        let reporter = CadenceReporter::for_host(host.clone());
        let report = crate::CadenceReport {
            observer: "test.observer".to_owned(),
            activation: "test.activation".to_owned(),
            interval: std::time::Duration::from_secs(1),
            scheduled_at: std::time::Duration::from_secs(1),
            started_at: Some(std::time::Duration::from_secs(1)),
            callback_duration: std::time::Duration::from_millis(40),
            skipped: false,
            refused: true,
        };
        supervisor.send_cadence(&report);
        supervisor.send_cadence(&report);
        for _ in 0..2 {
            let super::super::host::Outgoing::Frame(crate::Frame::Push { method, params }) = received.recv().await.unwrap() else {
                panic!("expected a cadence push");
            };
            assert_eq!(method, crate::Frame::OBSERVER_CADENCE);
            reporter.report(&crate::CadenceReport::decode(&params).unwrap());
        }
        assert_eq!(*host.0.lock().unwrap(), vec![report.clone(), report]);
    }
}
