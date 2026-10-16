/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The service kind of resident operation: a resident that runs until stopped
// under the resident half of its `OperationPolicy`. Environmental faults spend
// restart credit and restart after backoff; application faults deactivate it.
// The operation manager activates a service through `submit_with_resources`,
// and its context is the same `Host` every other body uses, lending the
// runtime's typed managers as context values.

/* === Modules === */
mod restart;
mod runner;

#[cfg(feature = "plugin-sidecar")]
mod sidecar;

/* === Re-Exports === */
pub(crate) use runner::{Once, Typed};

#[cfg(feature = "plugin-sidecar")]
pub(crate) use sidecar::Endpoint as NativeEndpoint;
#[cfg(feature = "plugin-sidecar")]
pub(crate) use sidecar::RuntimeDriver;

#[cfg(feature = "plugin-sidecar")]
pub use sidecar::{
    DemandLease, DemandSource, Managed, ManagedError, Notice, Publish, SidecarError, Supervised,
};

/* === Imports === */
// Standard Library Imports
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use crate::operations::{
    Backoff, CallContext, Callable, ContextError, ContextValue, Jitter, OperationInstance,
    OperationManager, OperationPolicy, Response, RestartPolicy, RetryPolicy,
};
use crate::supervision::executor;
use crate::{
    Application, Error, Result, evidence::Evidence, state::StateManager, streams::StreamManager,
};

/* === Constants === */
// Three restarts after environmental faults, refilled after a quarter second of
// health; `timeout` is the stop deadline.
pub const POLICY: OperationPolicy = OperationPolicy {
    timeout: Duration::from_secs(5),
    attempt_timeout: Duration::from_secs(5),
    retries: RetryPolicy::Never,
    restart: RestartPolicy::OnTransient {
        credit: 3,
        sustained_health: Duration::from_millis(250),
        backoff: Backoff::Exponential {
            initial: Duration::from_millis(10),
            maximum: Duration::from_secs(5),
            jitter: Jitter::Deterministic,
        },
    },
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct ServiceDefinition {
    pub id: String,
    pub policy: OperationPolicy,
}

// The runtime managers a service context lends as typed values.
pub(crate) struct Handles<A: Application> {
    pub(crate) storage:
        Arc<std::sync::OnceLock<crate::storage::StorageManager<crate::storage::ErasedCatalog>>>,
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::records::RecordManager<A>,
    pub(crate) streams: StreamManager<A>,
    pub(crate) evidence: Evidence<A>,
}

pub struct Service<A: Application> {
    pub(crate) state: StateManager<A>,
    pub(crate) records: crate::records::RecordManager<A>,
    pub(crate) operations: OperationManager,
    pub(crate) streams: StreamManager<A>,
    pub(crate) evidence: Evidence<A>,
    pub(crate) stopping: watch::Receiver<bool>,
    pub(crate) ready: watch::Sender<bool>,
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) provider: Arc<crate::supervision::instance::Instance>,
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
// A resident runs until stopped. Each run is stopped and cleaned up before the
// next restart or before the instance stops.
pub trait Resident<A: Application>: Send + 'static {
    type Error: fmt::Debug + fmt::Display + Send + Sync + 'static;

    fn run(
        &mut self,
        context: Service<A>,
    ) -> impl Future<Output = std::result::Result<(), Fault<Self::Error>>> + Send;

    fn stop(&mut self) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    fn cleanup(&mut self) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send;
}

/* === Implementations === */
impl ServiceDefinition {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            policy: POLICY,
        }
    }
}

impl<A: Application> Handles<A> {
    pub(crate) fn context(
        &self,
        operations: OperationManager,
        stopping: watch::Receiver<bool>,
        ready: watch::Sender<bool>,
        #[cfg(feature = "plugin-sidecar")] provider: Arc<crate::supervision::instance::Instance>,
    ) -> Service<A> {
        Service {
            state: self.state.clone(),
            records: self.records.clone(),
            operations,
            streams: self.streams.clone(),
            evidence: self.evidence.clone(),
            stopping,
            ready,
            #[cfg(feature = "plugin-sidecar")]
            provider,
        }
    }
}

impl<A: Application> Clone for Handles<A> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            state: self.state.clone(),
            records: self.records.clone(),
            streams: self.streams.clone(),
            evidence: self.evidence.clone(),
        }
    }
}

impl<A: Application> Service<A> {
    #[cfg(feature = "plugin-sidecar")]
    pub(crate) fn shared_attempt(&self) -> Self {
        Self {
            state: self.state.clone(),
            records: self.records.clone(),
            operations: self.operations.clone(),
            streams: self.streams.clone(),
            evidence: self.evidence.clone(),
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

    // An owned host handle for calls made while this context is borrowed,
    // such as inside `until_stopping`.
    pub fn host(&self) -> OperationManager {
        self.operations.clone()
    }

    pub fn records(&self) -> &crate::records::RecordManager<A> {
        &self.records
    }

    pub fn state(&self) -> &StateManager<A> {
        &self.state
    }

    pub fn streams(&self) -> &StreamManager<A> {
        &self.streams
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

impl<A: Application> harmos_contract::operations::Host for Service<A> {
    type Error = Error;
    type Instance<O: Callable> = OperationInstance<O>;

    fn context(&self) -> &CallContext {
        harmos_contract::operations::Host::context(&self.operations)
    }

    fn with_value<T: ContextValue>(&self, value: T) -> std::result::Result<Self, ContextError> {
        let operations = harmos_contract::operations::Host::with_value(&self.operations, value)?;
        let mut child = self.clone_context();
        child.operations = operations;
        Ok(child)
    }

    fn submit_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<OperationInstance<O>>> + Send {
        harmos_contract::operations::Host::submit_as::<O>(&self.operations, request)
    }

    fn call_as<O: Callable>(
        &self,
        request: O::Request,
    ) -> impl Future<Output = Result<Response<O::Response>>> + Send {
        harmos_contract::operations::Host::call_as::<O>(&self.operations, request)
    }

    fn submit_with_resources<O: Callable<Response = ()>, R: Send + Sync + 'static>(
        &self,
        request: O::Request,
        resources: Arc<R>,
    ) -> impl Future<Output = Result<OperationInstance<O>>> + Send {
        harmos_contract::operations::Host::submit_with_resources::<O, R>(
            &self.operations,
            request,
            resources,
        )
    }
}

impl<A: Application> Service<A> {
    fn clone_context(&self) -> Self {
        Self {
            state: self.state.clone(),
            records: self.records.clone(),
            operations: self.operations.clone(),
            streams: self.streams.clone(),
            evidence: self.evidence.clone(),
            stopping: self.stopping.clone(),
            ready: self.ready.clone(),
            #[cfg(feature = "plugin-sidecar")]
            provider: self.provider.clone(),
        }
    }
}

impl<A, F, Fut, E> Resident<A> for F
where
    A: Application,
    F: FnMut(Service<A>) -> Fut + Send + 'static,
    Fut: Future<Output = std::result::Result<(), Fault<E>>> + Send + 'static,
    E: fmt::Debug + fmt::Display + Send + Sync + 'static,
{
    type Error = E;

    fn run(
        &mut self,
        context: Service<A>,
    ) -> impl Future<Output = std::result::Result<(), Fault<Self::Error>>> + Send {
        self(context)
    }

    // Closures must not own asynchronous resources beyond their run future.
    // Resource-owning services implement Resident and retain their cleanup state.
    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
pub(crate) fn validate(definition: &ServiceDefinition) -> Result<()> {
    if definition.id.is_empty() {
        return Err(Error::InvalidInput {
            field: "service name",
            reason: "must not be empty",
        });
    }
    if definition.id == crate::evidence::SOURCE {
        return Err(Error::InvalidInput {
            field: "service name",
            reason: "artifact is reserved for evidence facts",
        });
    }
    definition
        .policy
        .validate()
        .map_err(|source| Error::OperationDeclaration {
            source: source.into(),
        })
}

/* === Tests === */
