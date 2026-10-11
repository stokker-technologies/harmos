/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod manager;
mod supervisor;

#[cfg(feature = "sidecar-host")]
mod sidecar;

/* === Re-Exports === */
pub use crate::supervision::instance::{
    Outcome as ServiceOutcome, Phase as ServicePhase, Status as ServiceLifecycle,
};
pub(crate) use manager::{Definition, Handles};
pub use manager::{
    ServiceDefinition, ServiceInstance, ServiceInstanceId, ServiceKey, ServiceLease,
    ServiceManager, ServiceScope, ServiceScopeId, ServiceSnapshot,
};

#[cfg(feature = "sidecar-host")]
pub(crate) use sidecar::Endpoint as NativeEndpoint;

#[cfg(feature = "sidecar-host")]
pub use sidecar::{
    DemandLease, DemandSource, Managed, ManagedError, Notice, Publish, SidecarError, Supervised,
};

/* === Imports === */
// Standard Library Imports
use std::fmt;
use std::future::Future;
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use crate::observers::ObserverManager;
use crate::supervision::executor;
use crate::{Application, Evidence, StateManager, StreamManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServicePolicy {
    pub credit: usize,
    pub sustained_health: Duration,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
    pub stop_timeout: Duration,
}

pub struct Service<A: Application> {
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::RecordManager<A>,
    pub(crate) operations: crate::OperationManager,
    pub(crate) streams: StreamManager<A>,
    pub(crate) evidence: Evidence<A>,
    pub(crate) observers: ObserverManager,
    pub(crate) stopping: watch::Receiver<bool>,
    pub(crate) ready: watch::Sender<bool>,
    #[cfg(feature = "sidecar-host")]
    pub(crate) provider: std::sync::Arc<crate::supervision::instance::Instance>,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Fault<E> {
    #[error("environmental fault: {0}")]
    Environmental(E),
    #[error("application fault: {0}")]
    Application(E),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServiceExit {
    #[error("application: {0}")]
    Application(String),
    #[error("resident panicked")]
    Panicked,
    #[error("restart credit exhausted")]
    CreditExhausted,
    #[error("resident stop deadline exceeded")]
    StopTimedOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceStatus {
    Starting,
    Running {
        credit: usize,
    },
    Stopping,
    BackingOff,
    Stopped,
    Inactive {
        cause: ServiceExit,
        history: Vec<String>,
    },
}

/* === Traits === */
pub trait Resident<A: Application>: Send + 'static {
    type Error: fmt::Debug + fmt::Display + Send + Sync + 'static;

    fn attempt(
        &mut self,
        context: Service<A>,
    ) -> impl Future<Output = Result<(), Fault<Self::Error>>> + Send;

    fn stop(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn finalize(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/* === Implementations === */
impl Default for ServicePolicy {
    fn default() -> Self {
        Self {
            credit: 3,
            sustained_health: Duration::from_millis(250),
            backoff_base: Duration::from_millis(10),
            backoff_cap: Duration::from_secs(5),
            stop_timeout: Duration::from_secs(5),
        }
    }
}

impl<A: Application> Service<A> {
    #[cfg(feature = "sidecar-host")]
    pub(crate) fn shared_attempt(&self) -> Self {
        Self {
            state: self.state.clone(),
            records: self.records.clone(),
            operations: self.operations.clone(),
            streams: self.streams.clone(),
            evidence: self.evidence.clone(),
            observers: self.observers.clone(),
            stopping: self.stopping.clone(),
            ready: self.ready.clone(),
            provider: self.provider.clone(),
        }
    }

    pub fn ready(&self) {
        if !self.is_stopping() {
            self.ready.send_replace(true);
        }
    }

    pub fn records(&self) -> &crate::RecordManager<A> {
        &self.records
    }

    pub fn operations(&self) -> &crate::OperationManager {
        &self.operations
    }

    pub fn state(&self) -> &StateManager<A> {
        &self.state
    }

    pub fn streams(&self) -> &StreamManager<A> {
        &self.streams
    }

    pub fn observers(&self) -> &ObserverManager {
        &self.observers
    }

    pub fn evidence(&self) -> &Evidence<A> {
        &self.evidence
    }

    pub fn is_stopping(&self) -> bool {
        *self.stopping.borrow()
    }

    pub async fn stopping(&mut self) {
        while !*self.stopping.borrow_and_update() {
            if self.stopping.changed().await.is_err() {
                break;
            }
        }
    }

    pub async fn rest(&mut self, duration: Duration) -> bool {
        self.until_stopping(executor::sleep(duration))
            .await
            .is_none()
    }

    pub async fn until_stopping<T>(&mut self, future: impl Future<Output = T>) -> Option<T> {
        if self.is_stopping() {
            return None;
        }
        let mut future = Box::pin(future);
        let mut stop = Box::pin(self.stopping.changed());

        std::future::poll_fn(|context| {
            if let Poll::Ready(Ok(()) | Err(_)) = stop.as_mut().poll(context) {
                return Poll::Ready(None);
            }
            future.as_mut().poll(context).map(Some)
        })
        .await
    }
}

impl<A, F, Fut, E> Resident<A> for F
where
    A: Application,
    F: FnMut(Service<A>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), Fault<E>>> + Send + 'static,
    E: fmt::Debug + fmt::Display + Send + Sync + 'static,
{
    type Error = E;

    fn attempt(
        &mut self,
        context: Service<A>,
    ) -> impl Future<Output = Result<(), Fault<Self::Error>>> + Send {
        self(context)
    }

    // Closures must not own asynchronous resources beyond their attempt future.
    // Resource-owning services implement Resident and retain their cleanup state.
    async fn finalize(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
