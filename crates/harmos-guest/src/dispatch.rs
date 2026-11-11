/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod configuration;
mod encoded;
#[cfg(test)]
mod registration_tests;

/* === Re-Exports === */

#[cfg(feature = "streaming")]
use harmos_operations::execution::operations::{WireStreamLimits, WireStreams};

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Poll, Waker};
// Third Party Imports

use tracing::Instrument;
// Local Imports
use crate::envelope::{WireAnswer, WireCommand, encode_prepared_grant};
use harmos_operations::execution::__private::{RegistrationBatch, ResourceSet};
use harmos_operations::execution::QualifiedId;
use harmos_operations::execution::operations::*;

/* === Constants === */

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

pub struct Table {
    owner: String,
    resources: ResourceSet,
    resources_frozen: bool,
    trusted_resources: ResourceSet,
    pending: Vec<RegistrationBatch>,
    catalogs: std::collections::HashSet<std::any::TypeId>,
    configuration_errors: Vec<StartupError>,
    binding_failed: bool,
    handlers: BTreeMap<String, Route>,
    retired: BTreeMap<String, u64>,
    services: BTreeMap<String, ServiceFactory>,
    prepared: BTreeMap<u64, Arc<Slot>>,
    capacity: Option<NonZeroUsize>,
    closed: bool,
    ordinary_closed: bool,
    provided: Vec<std::any::TypeId>,

    #[cfg(feature = "streaming")]
    stream_limits: Option<WireStreamLimits>,
}

struct Slot {
    cancellation: Cancellation,
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
    callees: Vec<harmos_operations::execution::operations::OperationDependency>,
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
    fn grant(&self) -> Option<harmos_operations::execution::plugin::Grant> {
        None
    }
    fn bind<'a>(&'a mut self, context: &'a ExecutionContext) -> Finished<'a>;
    fn attempt<'a>(&'a mut self, context: &'a ExecutionContext, epoch: Option<u64>) -> Answer<'a>;
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

impl Table {
    pub fn new(owner: &str) -> Result<Self, ServingError> {
        if !harmos_domain::identity::segment(owner) {
            return Err(ServingError::Catalog);
        }
        Ok(Self {
            owner: owner.to_owned(),
            resources: ResourceSet::new(),
            resources_frozen: false,
            trusted_resources: ResourceSet::new(),
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
            staged.install(factory, false)?;
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
        let catalog = OperationAssignments::default();
        catalog.install(assignments)?;
        for route in self.handlers.values() {
            for callee in &route.callees {
                let id = callee.id.as_str();
                let fingerprint = callee.fingerprint;
                let local = QualifiedId::qualify(&self.owner, id).map_err(|_| ServingError::Catalog)?;
                let actual = match self.handlers.get(local.as_str()) {
                    Some(entry) => entry.definition.fingerprint.0,
                    None => catalog.resolve_dependency(id, fingerprint)?.fingerprint,
                };
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
        if is_stream_traffic(&command) {
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
        // Hand Off Retired Owners So the Caller Can Drop Them Outside Its Lock
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
        if is_stream_traffic(&command) {
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
        #[cfg(not(feature = "streaming"))]
        if epoch.is_some() {
            return Err(ServingError::StreamUnsupported);
        }
        let mut context = context.clone();
        context.cancellation = self.cancellation.clone();
        self.active(loan.prepared.as_mut().expect("loaned invocation state").attempt(&context, epoch)).await
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

/* === Public Functions === */

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
            failure: match &error {
                ServingError::Startup(startup) => startup.report().into_failure(),
                _ => harmos_domain::Failure::originate(error.classification(), &error, harmos_domain::failure::Origin::Plugin),
            },
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

fn is_stream_traffic(command: &WireCommand) -> bool {
    matches!(command, WireCommand::StreamSend { .. } | WireCommand::StreamFinish { .. } | WireCommand::StreamReceive { .. })
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
