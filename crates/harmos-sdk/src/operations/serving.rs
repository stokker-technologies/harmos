/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod codec;
mod configuration;
mod owned;
#[cfg(feature = "grpc")]
pub(crate) mod protobuf;
#[cfg(feature = "streaming")]
mod streams;

/* === Re-Exports === */

pub use codec::WireCodecError;
#[cfg(feature = "streaming")]
pub use streams::{WireEndpoints, WireStreamLimits, WireStreams};

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Poll, Waker};
// Third Party Imports
use serde::{Deserialize, Serialize};
use tracing::Instrument;
// Local Imports
use super::{Contract, ContractError, ExecutionContext, FailureKind, OperationDefinition, RegistrationBatch, StartupError};
use crate::QualifiedId;

/* === Constants === */

pub const WIRE_METHOD: &str = "harmos.operation.v2";

pub const CHILD_METHOD: &str = "harmos.operation.child.v2";

/* === Macros === */

/* === Type Aliases === */

type Answer<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, ServingError>> + Send + 'a>>;
type Finished<'a> = Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + 'a>>;
type Finalization<S> = Pin<Box<dyn Future<Output = (S, Result<(), ServingError>)> + Send + 'static>>;
type Prepare = Box<dyn Fn(Vec<u8>, bool) -> Result<Box<dyn Prepared>, ServingError> + Send + Sync>;

/* === Structs === */

struct Contextual<F: std::future::Future> {
    future: Option<std::pin::Pin<Box<F>>>,
    dispatcher: tracing::Dispatch,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireChildCall {
    pub context: [u8; 32],

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_context: Option<[u8; 32]>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<super::CallValue>,

    pub operation: String,
    pub fingerprint: [u8; 32],
    pub generation: u64,
    pub payload: Vec<u8>,

    // Submitted children answer their instance id at admission and remain
    // owned by the calling attempt until joined, released or detached.
    #[serde(default)]
    pub submit: bool,
}

// Drives one child the same attempt submitted; the host resolves the instance
// only inside the attempt context that submitted it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireChildControl {
    pub context: [u8; 32],
    pub instance: u64,
    pub action: WireChildAction,
}

pub struct WireTable {
    owner: String,
    resources: super::ResourceSet,
    resources_frozen: bool,
    trusted_resources: super::ResourceSet,
    pending: Vec<RegistrationBatch>,
    catalogs: std::collections::HashSet<std::any::TypeId>,
    configuration_errors: Vec<StartupError>,
    binding_failed: bool,
    handlers: BTreeMap<String, Route>,
    retired: BTreeMap<String, u64>,
    services: BTreeMap<String, super::registration::ServiceFactory>,
    prepared: BTreeMap<u64, Arc<Slot>>,
    capacity: Option<NonZeroUsize>,
    closed: bool,
    ordinary_closed: bool,
    provided: Vec<std::any::TypeId>,

    #[cfg(feature = "streaming")]
    stream_limits: Option<WireStreamLimits>,
}

struct Slot {
    cancellation: super::Cancellation,
    operation: String,
    lifecycle: bool,
    resident: bool,

    #[cfg(feature = "streaming")]
    streams: Option<WireStreams>,

    state: Mutex<SlotState>,
}

struct SlotState {
    prepared: Option<Box<dyn Prepared>>,
    finished: bool,
    cancelled: bool,
    waiters: Vec<Weak<Mutex<Option<Waker>>>>,
}

struct Loan {
    slot: Arc<Slot>,
    prepared: Option<Box<dyn Prepared>>,
    finished: bool,
}

struct Route {
    callees: &'static [super::WireDeclaration],
    lifecycle: bool,
    // Lifecycle actions and the observe control are built in: they serve the
    // plugin's machinery and never appear in its declared catalog.
    builtin: bool,
    // A resident control runs until cancelled and is not charged against the
    // ordinary invocation capacity.
    resident: bool,
    definition: OperationDefinition,
    generation: Option<u64>,
    closing: bool,
    contract: Option<Contract>,
    prepare: Prepare,
}

/* === Enums === */

// Answers: `Join` the child's response payload, `Status` and `Cancel` an
// encoded instance state, `Release` and `Detach` an empty payload.
// Releasing cancels the child; detaching leaves it running without a handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireChildAction {
    Join,
    Status,
    Cancel,
    Release,
    Detach,
}

#[derive(Debug)]
pub enum WireChild {
    Call(WireChildCall),
    Control(WireChildControl),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireCommand {
    Prepare { invocation: u64, operation: String, fingerprint: [u8; 32], generation: u64, payload: Vec<u8> },
    Initialize { invocation: u64, context: [u8; 32] },
    Attempt { invocation: u64, context: [u8; 32], values: Vec<super::CallValue> },
    Finalize { invocation: u64 },
    StreamAttempt { invocation: u64, epoch: u64, context: [u8; 32], values: Vec<super::CallValue> },
    StreamSend { invocation: u64, epoch: u64, payload: Vec<u8> },
    StreamFinish { invocation: u64, epoch: u64 },
    StreamReceive { invocation: u64, epoch: u64 },
    Cancel { invocation: u64 },
    PrepareGrpc { invocation: u64, operation: String, fingerprint: [u8; 32], generation: u64, payload: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireAnswer {
    Stream { epoch: u64, transferred: bool, payload: Option<Vec<u8>> },
    Ready { payload: Vec<u8> },
    Failed { failure: crate::Failure },
}

#[derive(Debug, thiserror::Error)]
pub enum ServingError {
    #[error(transparent)]
    Startup(#[from] Box<StartupError>),

    #[error("the carrier resource store is frozen")]
    StoreFrozen,

    #[error("caller may not invoke operation `{operation}` without a host assignment")]
    Unassigned { operation: &'static str },

    #[error("operation endpoint types differ from the declaration")]
    EndpointTypes,

    #[error("carrier invocation does not support stream commands")]
    StreamUnsupported,

    #[cfg(feature = "streaming")]
    #[error("streaming catalog requires explicit provider stream limits")]
    StreamLimits,

    #[cfg(feature = "streaming")]
    #[error("carrier stream failed: {0}")]
    Stream(#[source] super::streaming::Error),

    #[cfg(feature = "streaming")]
    #[error("carrier stream direction already has an outstanding request")]
    StreamBusy,

    #[cfg(feature = "streaming")]
    #[error("carrier stream direction or attempt is not available")]
    StreamPhase,

    #[error(transparent)]
    Lifecycle(#[from] harmos_domain::model::operations::lifecycle::Error),

    #[error(transparent)]
    Contract(#[from] ContractError),

    #[error(transparent)]
    Declaration(#[from] crate::Error),

    #[error("operation `{operation}` declares resource `{resource}` that its registration does not provide")]
    Resource { operation: String, resource: &'static str },

    #[error("operation `{operation}` has no bound carrier handler")]
    Unknown { operation: String },

    #[error("operation `{operation}` requires streaming endpoints unsupported by this carrier binding")]
    Streaming { operation: String },

    #[error("operation `{operation}` has a mismatched fingerprint or stale generation")]
    Identity { operation: String },

    #[error("carrier invocation `{invocation}` is already prepared")]
    Duplicate { invocation: u64 },

    #[error("carrier invocation `{invocation}` has not been prepared")]
    Missing { invocation: u64 },

    #[error("carrier operation has not completed initialization")]
    Uninitialized,

    #[error("carrier operation initialization already started and cannot be replayed")]
    InitializationStarted,

    #[error("carrier invocation capacity is exhausted")]
    Capacity,

    #[error("carrier ordinary invocations have not finished settlement")]
    Unsettled,

    #[error("carrier operation admission is closing")]
    Closing,

    #[error("carrier handler definitions differ from the selected operation catalog")]
    Catalog,

    #[error("operation payload codec failed")]
    Codec(#[source] crate::MessageError),

    #[error("carrier handler panicked")]
    Panicked,

    // The handler's failure, written once at its origin.
    #[error(transparent)]
    Handler(crate::Failure),

    #[error("native handler failed: {0}")]
    Native(#[source] std::sync::Arc<dyn std::error::Error + Send + Sync>),
}

/* === Traits === */

trait InContext: std::future::Future + Sized {
    fn in_context(self, dispatcher: tracing::Dispatch) -> Contextual<Self> {
        Contextual { future: Some(Box::pin(self)), dispatcher }
    }

    fn in_current_context(self) -> Contextual<Self> {
        self.in_context(tracing::dispatcher::get_default(Clone::clone))
    }
}

trait Prepared: Send {
    #[cfg(feature = "streaming")]
    fn streams(&self) -> Option<WireStreams> {
        None
    }

    // The grant the prepared request needs, answered to the host with Prepare.
    fn grant(&self) -> Option<crate::plugin::Grant> {
        None
    }
    fn bind<'a>(&'a mut self, context: &'a ExecutionContext) -> Finished<'a>;
    fn attempt<'a>(&'a mut self, context: &'a ExecutionContext) -> Answer<'a>;
    fn finalize(&mut self) -> Finished<'_>;
}

/* === Implementations === */

impl<F: std::future::Future> InContext for F {}

impl<F: std::future::Future> std::future::Future for Contextual<F> {
    type Output = F::Output;

    fn poll(self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let owner = self.get_mut();
        let _dispatcher = tracing::dispatcher::set_default(&owner.dispatcher);
        owner.future.as_mut().expect("context future remains until drop").as_mut().poll(context)
    }
}

impl<F: std::future::Future> Drop for Contextual<F> {
    fn drop(&mut self) {
        tracing::dispatcher::with_default(&self.dispatcher, || drop(self.future.take()));
    }
}

impl WireTable {
    pub fn new(owner: &str) -> Result<Self, ServingError> {
        if !harmos_domain::identity::segment(owner) {
            return Err(ServingError::Catalog);
        }
        Ok(Self {
            owner: owner.to_owned(),
            resources: super::ResourceSet::new(),
            resources_frozen: false,
            trusted_resources: super::ResourceSet::new(),
            pending: Vec::new(),
            catalogs: std::collections::HashSet::new(),
            configuration_errors: Vec::new(),
            binding_failed: false,
            services: BTreeMap::new(),
            handlers: BTreeMap::new(),
            retired: BTreeMap::new(),
            prepared: BTreeMap::new(),
            capacity: None,
            closed: false,
            ordinary_closed: false,
            provided: Vec::new(),
            #[cfg(feature = "streaming")]
            stream_limits: None,
        })
    }

    // The Carrier Supplies These Resources To Every Attempt's Execution Context.
    pub fn provide(&mut self, resource: std::any::TypeId) {
        self.provided.push(resource);
    }

    pub fn provided(&self) -> &[std::any::TypeId] {
        &self.provided
    }

    pub fn capacity(&mut self, capacity: NonZeroUsize) {
        self.capacity = Some(capacity);
    }

    #[cfg(feature = "streaming")]
    pub fn stream_limits(&mut self, limits: WireStreamLimits) -> Result<(), ServingError> {
        limits.validate()?;
        self.stream_limits = Some(limits);
        Ok(())
    }

    #[doc(hidden)]
    pub fn register(&mut self, batch: RegistrationBatch) -> Result<(), ServingError> {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        if !self.resources_frozen {
            self.pending.push(batch);
            return Ok(());
        }
        self.register_now(batch)
    }

    fn register_now(&mut self, batch: RegistrationBatch) -> Result<(), ServingError> {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        let mut staged = self.configuration()?;
        staged.resources = self.resources.clone();
        staged.trusted_resources = self.trusted_resources.clone();
        for factory in batch.operations {
            factory.install(&mut staged)?;
        }
        for factory in batch.services {
            factory.policy.validate().map_err(|_| ServingError::Catalog)?;
            let id = QualifiedId::qualify(&self.owner, &factory.local_id).map_err(|_| ServingError::Catalog)?;
            let id = id.as_str().to_owned();
            if staged.handlers.contains_key(&id) || staged.services.insert(id, factory).is_some() {
                return Err(ServingError::Catalog);
            }
        }
        if staged.handlers.keys().chain(staged.services.keys()).any(|id| self.handlers.contains_key(id) || self.services.contains_key(id)) {
            return Err(ServingError::Catalog);
        }
        self.handlers.append(&mut staged.handlers);
        self.services.append(&mut staged.services);
        self.resources_frozen = true;
        Ok(())
    }

    pub async fn unregister(&mut self, id: &QualifiedId) -> Result<(), ServingError> {
        let key = id.as_str();
        if !self.handlers.contains_key(key) && !self.services.contains_key(key) {
            return Err(ServingError::Unknown { operation: key.to_owned() });
        }
        if let Some(handler) = self.handlers.get_mut(key) {
            handler.closing = true;
        }
        let slots: Vec<_> =
            self.prepared.iter().filter(|(_, slot)| slot.operation == key).map(|(invocation, slot)| (*invocation, slot.clone())).collect();
        for (_, slot) in &slots {
            slot.request_cancel();
        }
        for (_, slot) in &slots {
            slot.finalize().await?;
        }
        for (invocation, _) in slots {
            self.prepared.remove(&invocation);
        }
        if let Some(handler) = self.handlers.remove(key)
            && let Some(generation) = handler.generation
        {
            self.retired.insert(key.to_owned(), generation);
        }
        self.services.remove(key);
        Ok(())
    }

    pub fn definitions(&self) -> Vec<OperationDefinition> {
        self.handlers.values().filter(|handler| !handler.builtin).map(|handler| handler.definition.clone()).collect()
    }

    pub fn validate(&self, definitions: &[OperationDefinition]) -> Result<(), ServingError> {
        if definitions.len() != self.handlers.values().filter(|handler| !handler.builtin).count() {
            return Err(ServingError::Catalog);
        }
        for definition in definitions {
            if self
                .handlers
                .get(QualifiedId::qualify(&self.owner, definition.id.as_str()).map_err(|_| ServingError::Catalog)?.as_str())
                .is_none_or(|bound| bound.builtin || bound.definition != *definition)
            {
                return Err(ServingError::Catalog);
            }
        }
        let unique: std::collections::BTreeSet<_> = definitions.iter().map(|definition| &definition.id).collect();
        if unique.len() != definitions.len() {
            return Err(ServingError::Catalog);
        }
        Ok(())
    }

    pub fn validate_callees(&self, assignments: &[u8]) -> Result<(), ServingError> {
        let assignments: Vec<super::AssignedOperation> =
            if assignments.is_empty() { Vec::new() } else { postcard::from_bytes(assignments).map_err(|_| ServingError::Catalog)? };
        for route in self.handlers.values() {
            for callee in route.callees {
                let id = callee.operation.id;
                let fingerprint = (callee.operation.fingerprint)().map_err(|_| ServingError::Catalog)?;
                let local = QualifiedId::qualify(&self.owner, id).map_err(|_| ServingError::Catalog)?;
                let actual = self
                    .handlers
                    .get(local.as_str())
                    .map(|entry| entry.definition.fingerprint.0)
                    .or_else(|| {
                        assignments
                            .iter()
                            .find(|entry| entry.operation.split_once('.').is_some_and(|(_, name)| name == id))
                            .map(|entry| entry.fingerprint)
                    })
                    .ok_or(ServingError::Unassigned { operation: id })?;
                if actual != fingerprint.0 {
                    return Err(ServingError::Identity { operation: id.to_owned() });
                }
            }
        }
        Ok(())
    }

    pub fn validate_contracts(&self, contracts: &[Contract]) -> Result<(), ServingError> {
        if contracts.len() != self.handlers.values().filter(|handler| !handler.builtin).count() {
            return Err(ServingError::Catalog);
        }
        let mut unique = std::collections::BTreeSet::new();
        for contract in contracts {
            let id = &contract.operation().id;
            if !unique.insert(id)
                || self
                    .handlers
                    .get(QualifiedId::qualify(&self.owner, id.as_str()).map_err(|_| ServingError::Catalog)?.as_str())
                    .is_none_or(|bound| bound.builtin || bound.contract.as_ref() != Some(contract))
            {
                return Err(ServingError::Catalog);
            }
        }
        Ok(())
    }

    // Answers the request's grant as the Prepare payload.
    pub fn prepare(
        &mut self, invocation: u64, operation: &str, fingerprint: [u8; 32], generation: u64, payload: Vec<u8>,
    ) -> Result<Vec<u8>, ServingError> {
        self.prepare_codec(invocation, operation, fingerprint, generation, payload, false)
    }

    fn prepare_codec(
        &mut self, invocation: u64, operation: &str, fingerprint: [u8; 32], generation: u64, payload: Vec<u8>, grpc: bool,
    ) -> Result<Vec<u8>, ServingError> {
        if self.closed {
            return Err(ServingError::Closing);
        }
        self.prepared.retain(|_, slot| !slot.is_finished());
        if self.prepared.contains_key(&invocation) {
            return Err(ServingError::Duplicate { invocation });
        }
        let handler = self.handlers.get_mut(operation).ok_or_else(|| ServingError::Unknown { operation: operation.to_owned() })?;
        if handler.definition.fingerprint.0 != fingerprint
            || generation == 0
            || handler.generation.is_some_and(|bound| bound != generation)
            || self.retired.get(operation).is_some_and(|retired| generation <= *retired)
        {
            return Err(ServingError::Identity { operation: operation.to_owned() });
        }
        // The authenticated host assigns a generation with its first preparation.
        // Keep it through withdrawal so delayed preparations cannot bind a replacement.
        handler.generation = Some(generation);
        if handler.closing || (self.ordinary_closed && !handler.lifecycle) {
            return Err(ServingError::Closing);
        }
        let occupied = self.prepared.values().filter(|slot| slot.lifecycle == handler.lifecycle && !slot.resident).count();
        // One accounted reserved lifecycle slot remains available even when
        // ordinary admission is full. Failed SDK finalization retains that slot.
        let capacity = if handler.lifecycle { 1 } else { self.capacity.ok_or(ServingError::Capacity)?.get() };
        if occupied >= capacity && !handler.resident {
            return Err(ServingError::Capacity);
        }
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (handler.prepare)(payload, grpc)))
            .map_err(|_| ServingError::Panicked)??;
        let answer = encode_prepared_grant(prepared.grant().as_ref());
        let slot = Slot {
            cancellation: Default::default(),
            operation: operation.to_owned(),
            lifecycle: handler.lifecycle,
            resident: handler.resident,
            #[cfg(feature = "streaming")]
            streams: prepared.streams(),
            state: Mutex::new(SlotState {
                prepared: Some(prepared),
                finished: false,
                cancelled: false,
                waiters: Vec::new(),
            }),
        };
        self.prepared.insert(invocation, Arc::new(slot));
        Ok(answer)
    }

    #[cfg(feature = "streaming")]
    pub fn streams(&self, invocation: u64) -> Result<WireStreams, ServingError> {
        let slot = self.prepared.get(&invocation).ok_or(ServingError::Missing { invocation })?;
        if self.closed || (self.ordinary_closed && !slot.lifecycle) || slot.is_finished() {
            return Err(ServingError::Closing);
        }
        slot.streams.clone().ok_or(ServingError::StreamPhase)
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.initialize", level = "trace", skip_all, fields(invocation))]
    pub async fn bind(&mut self, invocation: u64, context: &ExecutionContext) -> Result<(), ServingError> {
        if self.closed {
            return Err(ServingError::Closing);
        }
        let slot = self.prepared.get(&invocation).ok_or(ServingError::Missing { invocation })?;
        if self.ordinary_closed && !slot.lifecycle {
            return Err(ServingError::Closing);
        }
        slot.bind(invocation, context).await
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.attempt", level = "trace", skip_all, fields(invocation))]
    pub async fn attempt(&mut self, invocation: u64, context: &ExecutionContext) -> Result<Vec<u8>, ServingError> {
        if self.closed {
            return Err(ServingError::Closing);
        }
        let slot = self.prepared.get(&invocation).ok_or(ServingError::Missing { invocation })?;
        if self.ordinary_closed && !slot.lifecycle {
            return Err(ServingError::Closing);
        }
        slot.attempt(invocation, context, None).await
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.finalize", level = "trace", skip_all, fields(invocation))]
    pub async fn finalize(&mut self, invocation: u64) -> Result<(), ServingError> {
        if let Some(slot) = self.prepared.get(&invocation) {
            slot.finalize().await?;
        }
        self.prepared.remove(&invocation);
        Ok(())
    }

    fn traffic(&self, command: WireCommand) -> Pin<Box<dyn Future<Output = WireAnswer> + Send + 'static>> {
        #[cfg(not(feature = "streaming"))]
        {
            let _ = command;
            Box::pin(async { wire_answer(Err(ServingError::StreamUnsupported)) })
        }
        #[cfg(feature = "streaming")]
        {
            let invocation = match &command {
                WireCommand::StreamSend { invocation, .. }
                | WireCommand::StreamFinish { invocation, .. }
                | WireCommand::StreamReceive { invocation, .. } => *invocation,
                _ => unreachable!("only stream traffic bypasses the phase loan"),
            };
            let streams = self.streams(invocation);
            Box::pin(async move {
                let streams = match streams {
                    Ok(streams) => streams,
                    Err(error) => return wire_answer(Err(error)),
                };
                match command {
                    WireCommand::StreamSend { epoch, payload, .. } => {
                        wire_answer(guarded(async { streams.send(epoch, &payload).await.map(|()| Vec::new()) }).await)
                    }
                    WireCommand::StreamFinish { epoch, .. } => wire_answer(streams.finish_input(epoch).map(|()| Vec::new())),
                    WireCommand::StreamReceive { epoch, .. } => match guarded(streams.receive(epoch)).await {
                        Ok(payload) => WireAnswer::Stream {
                            epoch,
                            transferred: streams.transferred(),
                            payload,
                        },
                        Err(error) => wire_answer(Err(error)),
                    },
                    _ => unreachable!("only stream traffic bypasses the phase loan"),
                }
            })
        }
    }

    fn signal(&self, invocation: u64) -> Result<Vec<u8>, ServingError> {
        let slot = self.prepared.get(&invocation).ok_or(ServingError::Missing { invocation })?;
        slot.cancellation.request();
        Ok(Vec::new())
    }

    pub fn begin(
        &mut self, command: WireCommand, context: Arc<ExecutionContext>,
    ) -> Pin<Box<dyn Future<Output = WireAnswer> + Send + 'static>> {
        let (invocation, phase) = command_identity(&command);
        let span = tracing::debug_span!(target: "harmos::carrier", "carrier.receive", invocation, phase, outcome = tracing::field::Empty);
        if command.is_stream_traffic() {
            return Box::pin(self.traffic(command).instrument(span).in_current_context());
        }
        if let WireCommand::Cancel { invocation } = command {
            let result = self.signal(invocation);
            return Box::pin(async move { wire_answer(result) });
        }
        let slot = match &command {
            WireCommand::Initialize { invocation, .. }
            | WireCommand::Attempt { invocation, .. }
            | WireCommand::StreamAttempt { invocation, .. }
            | WireCommand::Finalize { invocation } => self.prepared.get(invocation).cloned(),
            WireCommand::Prepare { .. } | WireCommand::PrepareGrpc { .. } | WireCommand::Cancel { .. } => None,
            WireCommand::StreamSend { .. } | WireCommand::StreamFinish { .. } | WireCommand::StreamReceive { .. } => {
                unreachable!("stream traffic already dispatched")
            }
        };
        let command = match command {
            WireCommand::PrepareGrpc {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => {
                let result = span.in_scope(|| self.prepare_codec(invocation, &operation, fingerprint, generation, payload, true));
                return Box::pin(async move { wire_answer(result) }.instrument(span).in_current_context());
            }
            WireCommand::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => {
                let result = span.in_scope(|| self.prepare(invocation, &operation, fingerprint, generation, payload));
                return Box::pin(async move { wire_answer(result) }.instrument(span).in_current_context());
            }
            command => command,
        };
        let refused = self.closed || (self.ordinary_closed && slot.as_ref().is_some_and(|slot| !slot.lifecycle));
        if refused && matches!(command, WireCommand::Initialize { .. } | WireCommand::Attempt { .. } | WireCommand::StreamAttempt { .. }) {
            return Box::pin(async { wire_answer(Err(ServingError::Closing)) }.instrument(span).in_current_context());
        }
        Box::pin(
            async move {
                let result = match command {
                    WireCommand::Initialize { invocation, .. } => match slot {
                        Some(slot) => slot.bind(invocation, &context).await.map(|()| Vec::new()),
                        None => Err(ServingError::Missing { invocation }),
                    },
                    WireCommand::Attempt { invocation, .. } => match slot {
                        Some(slot) => slot.attempt(invocation, &context, None).await,
                        None => Err(ServingError::Missing { invocation }),
                    },
                    WireCommand::StreamAttempt { invocation, epoch, .. } => match slot {
                        Some(slot) => slot.attempt(invocation, &context, Some(epoch)).await,
                        None => Err(ServingError::Missing { invocation }),
                    },
                    WireCommand::StreamSend { .. } | WireCommand::StreamFinish { .. } | WireCommand::StreamReceive { .. } => {
                        unreachable!("stream traffic already dispatched")
                    }
                    WireCommand::Finalize { .. } => match slot {
                        Some(slot) => slot.finalize().await.map(|()| Vec::new()),
                        None => Ok(Vec::new()),
                    },
                    WireCommand::Prepare { .. } | WireCommand::PrepareGrpc { .. } | WireCommand::Cancel { .. } => {
                        unreachable!("preparation is synchronous")
                    }
                };
                wire_answer(result)
            }
            .instrument(span)
            .in_current_context(),
        )
    }

    pub fn close_ordinary(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + 'static>> {
        self.ordinary_closed = true;
        let slots: Vec<_> = self.prepared.values().filter(|slot| !slot.lifecycle).cloned().collect();
        for slot in &slots {
            slot.request_cancel();
        }
        Box::pin(async move {
            for slot in slots {
                slot.finalize().await?;
            }
            Ok(())
        })
    }

    pub fn release_ordinary(&mut self) -> Result<impl Send + use<>, ServingError> {
        if !self.ordinary_closed || self.prepared.values().any(|slot| !slot.lifecycle && !slot.is_finished()) {
            return Err(ServingError::Unsettled);
        }
        // Hand Off Retired Owners So The Caller Can Drop Them Outside Its Lock
        let retired_slots = self.prepared.extract_if(.., |_, slot| !slot.lifecycle).collect::<Vec<_>>();
        let retired_handlers = self.handlers.extract_if(.., |_, handler| !handler.lifecycle).collect::<Vec<_>>();
        let retired_services = std::mem::take(&mut self.services);
        Ok((retired_slots, retired_handlers, retired_services))
    }

    pub fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + 'static>> {
        self.closed = true;
        let slots: Vec<_> = self.prepared.values().cloned().collect();
        for slot in &slots {
            slot.request_cancel();
        }
        Box::pin(async move {
            for slot in slots {
                slot.finalize().await?;
            }
            Ok(())
        })
    }

    pub fn release(&mut self) -> Result<impl Send + use<>, ServingError> {
        if !self.closed || self.prepared.values().any(|slot| !slot.is_finished()) {
            return Err(ServingError::Unsettled);
        }
        Ok((std::mem::take(&mut self.prepared), std::mem::take(&mut self.handlers), std::mem::take(&mut self.services)))
    }

    pub async fn settle(&mut self) -> Result<(), ServingError> {
        self.close().await?;
        let retired = self.release()?;
        drop(retired);
        Ok(())
    }

    pub fn is_settled(&self) -> bool {
        self.closed && self.prepared.values().all(|slot| slot.is_finished())
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    #[tracing::instrument(target = "harmos::carrier", name = "carrier.answer", level = "debug", skip_all, fields(invocation = command_identity(&command).0, phase = command_identity(&command).1))]
    pub async fn answer(&mut self, command: WireCommand, context: &ExecutionContext) -> WireAnswer {
        if command.is_stream_traffic() {
            return self.traffic(command).await;
        }
        if let WireCommand::StreamAttempt { invocation, epoch, .. } = command {
            if self.closed || (self.ordinary_closed && self.prepared.get(&invocation).is_some_and(|slot| !slot.lifecycle)) {
                return wire_answer(Err(ServingError::Closing));
            }
            let Some(slot) = self.prepared.get(&invocation) else {
                return wire_answer(Err(ServingError::Missing { invocation }));
            };
            return wire_answer(slot.attempt(invocation, context, Some(epoch)).await);
        }
        let result = match command {
            WireCommand::Cancel { invocation } => self.signal(invocation),
            WireCommand::PrepareGrpc {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => self.prepare_codec(invocation, &operation, fingerprint, generation, payload, true),
            WireCommand::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            } => self.prepare(invocation, &operation, fingerprint, generation, payload),
            WireCommand::Initialize { invocation, .. } => self.bind(invocation, context).await.map(|()| Vec::new()),
            WireCommand::Attempt { invocation, .. } => self.attempt(invocation, context).await,
            WireCommand::StreamAttempt { .. }
            | WireCommand::StreamSend { .. }
            | WireCommand::StreamFinish { .. }
            | WireCommand::StreamReceive { .. } => {
                unreachable!("stream command already dispatched")
            }
            WireCommand::Finalize { invocation } => self.finalize(invocation).await.map(|()| Vec::new()),
        };
        wire_answer(result)
    }
}

impl Slot {
    fn request_cancel(&self) {
        let waiters = {
            let mut state = self.state.lock().expect("invocation state lock");
            state.cancelled = true;
            std::mem::take(&mut state.waiters)
        };
        #[cfg(feature = "streaming")]
        if let Some(streams) = &self.streams {
            streams.close();
        }
        for waiter in waiters {
            if let Some(waiter) = waiter.upgrade() {
                let wake = waiter.lock().expect("invocation waiter lock").take();
                if let Some(wake) = wake {
                    wake.wake();
                }
            }
        }
    }

    async fn active<T>(&self, future: impl Future<Output = Result<T, ServingError>>) -> Result<T, ServingError> {
        let mut future = std::pin::pin!(future);
        let waiter = Arc::new(Mutex::new(None));
        std::future::poll_fn(|context| {
            {
                let mut state = self.state.lock().expect("invocation state lock");
                if state.cancelled {
                    return Poll::Ready(Err(ServingError::Closing));
                }
                *waiter.lock().expect("invocation waiter lock") = Some(context.waker().clone());
                state.waiters.retain(|entry| entry.strong_count() != 0);
                if !state.waiters.iter().any(|entry| entry.ptr_eq(&Arc::downgrade(&waiter))) {
                    state.waiters.push(Arc::downgrade(&waiter));
                }
            }
            future.as_mut().poll(context)
        })
        .await
    }

    fn is_finished(&self) -> bool {
        self.state.lock().expect("invocation state lock").finished
    }

    async fn borrow(self: &Arc<Self>) -> Option<Loan> {
        let waiter = Arc::new(Mutex::new(None));
        std::future::poll_fn(|context| {
            let mut state = self.state.lock().expect("invocation state lock");
            if state.finished {
                return Poll::Ready(None);
            }
            if let Some(prepared) = state.prepared.take() {
                return Poll::Ready(Some(Loan {
                    slot: Arc::clone(self),
                    prepared: Some(prepared),
                    finished: false,
                }));
            }
            *waiter.lock().expect("invocation waiter lock") = Some(context.waker().clone());
            state.waiters.retain(|entry| entry.strong_count() != 0);
            if !state.waiters.iter().any(|entry| entry.ptr_eq(&Arc::downgrade(&waiter))) {
                state.waiters.push(Arc::downgrade(&waiter));
            }
            Poll::Pending
        })
        .await
    }
}

impl Slot {
    #[tracing::instrument(target = "harmos::streams", name = "streams.initialize", level = "trace", skip_all, fields(invocation))]
    async fn bind(self: &Arc<Self>, invocation: u64, context: &ExecutionContext) -> Result<(), ServingError> {
        let mut loan = self.borrow().await.ok_or(ServingError::Missing { invocation })?;
        self.active(loan.prepared.as_mut().expect("loaned invocation state").bind(context)).await
    }

    #[tracing::instrument(target = "harmos::streams", name = "streams.attempt", level = "trace", skip_all, fields(invocation))]
    async fn attempt(self: &Arc<Self>, invocation: u64, context: &ExecutionContext, epoch: Option<u64>) -> Result<Vec<u8>, ServingError> {
        let mut loan = self.borrow().await.ok_or(ServingError::Missing { invocation })?;
        #[cfg(feature = "streaming")]
        let attempt = match (&self.streams, epoch) {
            (Some(streams), Some(epoch)) => Some(streams.start(epoch)?),
            (None, None) => None,
            _ => return Err(ServingError::StreamUnsupported),
        };
        #[cfg(not(feature = "streaming"))]
        if epoch.is_some() {
            return Err(ServingError::StreamUnsupported);
        }
        let mut context = context.clone();
        context.cancellation = self.cancellation.clone();
        let result = self.active(loan.prepared.as_mut().expect("loaned invocation state").attempt(&context)).await;
        #[cfg(feature = "streaming")]
        if let Some(attempt) = attempt {
            attempt.end();
        }
        result
    }

    #[tracing::instrument(target = "harmos::carrier", name = "carrier.finalize", level = "debug", skip_all)]
    async fn finalize(self: &Arc<Self>) -> Result<(), ServingError> {
        #[cfg(feature = "streaming")]
        if let Some(streams) = &self.streams {
            streams.close();
        }
        let Some(mut loan) = self.borrow().await else {
            return Ok(());
        };
        loan.prepared.as_mut().expect("loaned invocation state").finalize().await?;
        // Drop Physical State Before Releasing Its Capacity
        loan.finished = true;
        let prepared = loan.prepared.take();
        drop(prepared);
        Ok(())
    }
}

impl Drop for Loan {
    fn drop(&mut self) {
        let waiters = {
            let mut state = self.slot.state.lock().expect("invocation state lock");
            state.prepared = self.prepared.take();
            state.finished = self.finished;
            std::mem::take(&mut state.waiters)
        };
        for waiter in waiters {
            if let Some(waiter) = waiter.upgrade() {
                let wake = waiter.lock().expect("invocation waiter lock").take();
                if let Some(wake) = wake {
                    wake.wake();
                }
            }
        }
    }
}

impl WireCommand {
    pub const LIMIT: usize = codec::LIMIT;
    fn is_stream_traffic(&self) -> bool {
        matches!(self, Self::StreamSend { .. } | Self::StreamFinish { .. } | Self::StreamReceive { .. })
    }
}

impl ServingError {
    pub fn classification(&self) -> FailureKind {
        match self {
            Self::Handler(failure) => failure.code(),
            Self::Panicked => FailureKind::Ambiguous,
            Self::Codec(_) | Self::Declaration(_) | Self::Lifecycle(_) | Self::Contract(_) | Self::Catalog | Self::Streaming { .. } => {
                FailureKind::Invalid
            }
            _ => FailureKind::Permanent,
        }
    }
}

/* === Public Functions === */

// A Prepare answer is empty when the request needs no grant, else the grant as JSON.
pub fn encode_prepared_grant(grant: Option<&crate::plugin::Grant>) -> Vec<u8> {
    grant.map_or_else(Vec::new, |grant| serde_json::to_vec(grant).expect("a grant serializes to JSON"))
}

pub fn decode_prepared_grant(payload: &[u8]) -> Result<Option<crate::plugin::Grant>, serde_json::Error> {
    if payload.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(payload).map(Some)
}

/* === Private Functions === */

fn command_identity(command: &WireCommand) -> (u64, &'static str) {
    match command {
        WireCommand::Cancel { invocation } => (*invocation, "cancel"),
        WireCommand::Prepare { invocation, .. } => (*invocation, "prepare"),
        WireCommand::PrepareGrpc { invocation, .. } => (*invocation, "prepare_grpc"),
        WireCommand::Initialize { invocation, .. } => (*invocation, "initialize"),
        WireCommand::Attempt { invocation, .. } => (*invocation, "attempt"),
        WireCommand::Finalize { invocation, .. } => (*invocation, "finalize"),
        WireCommand::StreamAttempt { invocation, .. } => (*invocation, "stream_attempt"),
        WireCommand::StreamSend { invocation, .. } => (*invocation, "stream_send"),
        WireCommand::StreamFinish { invocation, .. } => (*invocation, "stream_finish"),
        WireCommand::StreamReceive { invocation, .. } => (*invocation, "stream_receive"),
    }
}

fn wire_answer(result: Result<Vec<u8>, ServingError>) -> WireAnswer {
    tracing::Span::current().record("outcome", if result.is_ok() { "ready" } else { "failed" });
    match result {
        Ok(payload) => WireAnswer::Ready { payload },
        Err(ServingError::Handler(failure)) => WireAnswer::Failed { failure },
        Err(error) => WireAnswer::Failed {
            failure: crate::Failure::originate(error.classification(), &error, crate::failure::Origin::Plugin),
        },
    }
}

async fn guarded<T>(future: impl Future<Output = Result<T, ServingError>>) -> Result<T, ServingError> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(context))) {
        Ok(result) => result,
        Err(_) => std::task::Poll::Ready(Err(ServingError::Panicked)),
    })
    .await
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn withdrawal_cancels_a_pending_active_future() {
        let slot = Slot {
            cancellation: Default::default(),
            operation: "host.read".to_owned(),
            lifecycle: false,
            resident: false,
            #[cfg(feature = "streaming")]
            streams: None,
            state: Mutex::new(SlotState {
                prepared: None,
                finished: false,
                cancelled: false,
                waiters: Vec::new(),
            }),
        };
        let mut active = Box::pin(slot.active(std::future::pending::<Result<(), ServingError>>()));
        let mut context = std::task::Context::from_waker(Waker::noop());
        assert!(active.as_mut().poll(&mut context).is_pending());
        slot.request_cancel();
        assert!(matches!(active.as_mut().poll(&mut context), Poll::Ready(Err(ServingError::Closing))));
    }
}
