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
mod owned;
#[cfg(feature = "grpc")]
mod protobuf;
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
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Poll, Waker};
// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::{
    AuthoredDeclarations, Contract, ContractError, Definition, Failure, Operation,
    OperationDeclaration, WireOperation,
};
use crate::Message;
use crate::catalog::{Catalog, Member, Projection, Visitor};
use crate::schema::Described;

/* === Constants === */
pub const WIRE_METHOD: &str = "harmos.operation.v2";

pub const CHILD_METHOD: &str = "harmos.operation.child.v2";

/* === Macros === */

/* === Type Aliases === */
type Answer<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, ServingError>> + Send + 'a>>;
type Finished<'a> = Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + 'a>>;
type Finalization<S> =
    Pin<Box<dyn Future<Output = (S, Result<(), ServingError>)> + Send + 'static>>;
type Prepare<C> =
    Box<dyn Fn(Vec<u8>, bool) -> Result<Box<dyn Prepared<C>>, ServingError> + Send + Sync>;
type Install<H, C> = fn(&mut WireTable<C>, Arc<H>) -> Result<(), ServingError>;

/* === Structs === */
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireChildCall {
    pub context: [u8; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_context: Option<[u8; 32]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<super::CallValue>,
    pub operation: String,
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

pub struct WireTable<C> {
    handlers: BTreeMap<String, Handler<C>>,
    prepared: BTreeMap<u64, Arc<Slot<C>>>,
    capacity: NonZeroUsize,
    closed: bool,
    ordinary_closed: bool,
    #[cfg(feature = "streaming")]
    stream_limits: Option<WireStreamLimits>,
}

struct Slot<C> {
    lifecycle: bool,
    resident: bool,
    #[cfg(feature = "streaming")]
    streams: Option<WireStreams>,
    state: Mutex<SlotState<C>>,
}

struct SlotState<C> {
    prepared: Option<Box<dyn Prepared<C>>>,
    finished: bool,
    waiters: Vec<Weak<Mutex<Option<Waker>>>>,
}

struct Loan<C> {
    slot: Arc<Slot<C>>,
    prepared: Option<Box<dyn Prepared<C>>>,
    finished: bool,
}

struct Handler<C> {
    lifecycle: bool,
    // A resident control runs until cancelled and is not charged against the
    // ordinary invocation capacity.
    resident: bool,
    definition: Definition,
    contract: Option<Contract>,
    prepare: Prepare<C>,
}

pub struct Serves<H, C>(PhantomData<fn() -> (H, C)>);

pub struct WireBinding<H, C> {
    install: Install<H, C>,
}

/* === Enums === */
// Answers: `Join` the child's response payload, `Status` and `Cancel` an
// encoded `builtin::InstanceState`, `Release` and `Detach` an empty payload.
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
    Prepare {
        invocation: u64,
        operation: String,
        payload: Vec<u8>,
    },
    Initialize {
        invocation: u64,
        context: [u8; 32],
    },
    Attempt {
        invocation: u64,
        context: [u8; 32],
        values: Vec<super::CallValue>,
    },
    Finalize {
        invocation: u64,
    },
    StreamAttempt {
        invocation: u64,
        epoch: u64,
        context: [u8; 32],
        values: Vec<super::CallValue>,
    },
    StreamSend {
        invocation: u64,
        epoch: u64,
        payload: Vec<u8>,
    },
    StreamFinish {
        invocation: u64,
        epoch: u64,
    },
    StreamReceive {
        invocation: u64,
        epoch: u64,
    },
    PrepareGrpc {
        invocation: u64,
        operation: String,
        payload: Vec<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum WireAnswer {
    Stream {
        epoch: u64,
        transferred: bool,
        payload: Option<Vec<u8>>,
    },
    Ready {
        payload: Vec<u8>,
    },
    Failed {
        failure: Failure,
        message: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ServingError {
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
    Lifecycle(#[from] super::lifecycle::Error),
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error(transparent)]
    Declaration(#[from] crate::Error),
    #[error("operation `{operation}` has no bound carrier handler")]
    Unknown { operation: String },
    #[error(
        "operation `{operation}` requires streaming endpoints unsupported by this carrier binding"
    )]
    Streaming { operation: String },
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
    #[error("carrier handler failed: {source}")]
    Handler {
        failure: Failure,
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

/* === Traits === */
trait Prepared<C>: Send {
    #[cfg(feature = "streaming")]
    fn streams(&self) -> Option<WireStreams> {
        None
    }
    // The grant the prepared request needs, answered to the host with Prepare.
    fn grant(&self) -> Option<crate::plugin::Grant> {
        None
    }
    fn initialize<'a>(&'a mut self, context: &'a C) -> Finished<'a>;
    fn attempt<'a>(&'a mut self, context: &'a C) -> Answer<'a>;
    fn finalize(&mut self) -> Finished<'_>;
}

/* === Implementations === */
impl<C: Send + Sync + 'static> WireTable<C> {
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            handlers: BTreeMap::new(),
            prepared: BTreeMap::new(),
            capacity,
            closed: false,
            ordinary_closed: false,
            #[cfg(feature = "streaming")]
            stream_limits: None,
        }
    }

    pub fn of<T, R>(resources: Arc<R>, capacity: NonZeroUsize) -> Result<Self, ServingError>
    where
        T: Catalog<AuthoredDeclarations> + Catalog<Serves<R, C>>,
        R: Send + Sync + 'static,
    {
        Self::project::<T, R>(Self::new(capacity), resources)
    }

    fn project<T, R>(mut table: Self, resources: Arc<R>) -> Result<Self, ServingError>
    where
        T: Catalog<AuthoredDeclarations> + Catalog<Serves<R, C>>,
        R: Send + Sync + 'static,
    {
        let definitions = AuthoredDeclarations::of::<T>()?;
        <T as Catalog<Serves<R, C>>>::ENTRIES
            .try_for_each(|binding| (binding.install)(&mut table, Arc::clone(&resources)))?;
        table.validate(&definitions)?;
        Ok(table)
    }

    pub fn bind_operations<T, R>(&mut self, resources: Arc<R>) -> Result<(), ServingError>
    where
        T: Catalog<AuthoredDeclarations> + Catalog<Serves<R, C>>,
        R: Send + Sync + 'static,
    {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        let staged = Self::of::<T, R>(resources, self.capacity)?;
        self.extend(staged)
    }

    #[cfg(feature = "streaming")]
    pub fn bind_streaming_operations<T, R>(
        &mut self,
        resources: Arc<R>,
        limits: WireStreamLimits,
    ) -> Result<(), ServingError>
    where
        T: Catalog<AuthoredDeclarations> + Catalog<Serves<R, C>>,
        R: Send + Sync + 'static,
    {
        if self.closed || self.ordinary_closed {
            return Err(ServingError::Closing);
        }
        limits.validate()?;
        let mut staged = Self::new(self.capacity);
        staged.stream_limits = Some(limits);
        let staged = Self::project::<T, R>(staged, resources)?;
        self.extend(staged)
    }

    fn extend(&mut self, mut staged: Self) -> Result<(), ServingError> {
        for id in staged.handlers.keys() {
            if self.handlers.contains_key(id) {
                return Err(crate::Error::DuplicateOperation { id: id.clone() }.into());
            }
        }
        self.handlers.append(&mut staged.handlers);
        Ok(())
    }

    pub fn definitions(&self) -> Vec<Definition> {
        self.handlers
            .values()
            .filter(|handler| !handler.lifecycle)
            .map(|handler| handler.definition.clone())
            .collect()
    }

    pub fn validate(&self, definitions: &[Definition]) -> Result<(), ServingError> {
        if definitions.len()
            != self
                .handlers
                .values()
                .filter(|handler| !handler.lifecycle)
                .count()
        {
            return Err(ServingError::Catalog);
        }
        for definition in definitions {
            if self
                .handlers
                .get(definition.id.as_str())
                .is_none_or(|bound| bound.lifecycle || bound.definition != *definition)
            {
                return Err(ServingError::Catalog);
            }
        }
        let unique: std::collections::BTreeSet<_> = definitions
            .iter()
            .map(|definition| &definition.id)
            .collect();
        if unique.len() != definitions.len() {
            return Err(ServingError::Catalog);
        }
        Ok(())
    }

    pub fn validate_contracts(&self, contracts: &[Contract]) -> Result<(), ServingError> {
        if contracts.len()
            != self
                .handlers
                .values()
                .filter(|handler| !handler.lifecycle)
                .count()
        {
            return Err(ServingError::Catalog);
        }
        let mut unique = std::collections::BTreeSet::new();
        for contract in contracts {
            let id = &contract.operation().id;
            if !unique.insert(id)
                || self.handlers.get(id.as_str()).is_none_or(|bound| {
                    bound.lifecycle || bound.contract.as_ref() != Some(contract)
                })
            {
                return Err(ServingError::Catalog);
            }
        }
        Ok(())
    }

    // Answers the request's grant as the Prepare payload.
    pub fn prepare(
        &mut self,
        invocation: u64,
        operation: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, ServingError> {
        self.prepare_codec(invocation, operation, payload, false)
    }

    fn prepare_codec(
        &mut self,
        invocation: u64,
        operation: &str,
        payload: Vec<u8>,
        grpc: bool,
    ) -> Result<Vec<u8>, ServingError> {
        if self.closed {
            return Err(ServingError::Closing);
        }
        self.prepared.retain(|_, slot| !slot.is_finished());
        if self.prepared.contains_key(&invocation) {
            return Err(ServingError::Duplicate { invocation });
        }
        let handler = self
            .handlers
            .get(operation)
            .ok_or_else(|| ServingError::Unknown {
                operation: operation.to_owned(),
            })?;
        if self.ordinary_closed && !handler.lifecycle {
            return Err(ServingError::Closing);
        }
        let occupied = self
            .prepared
            .values()
            .filter(|slot| slot.lifecycle == handler.lifecycle && !slot.resident)
            .count();
        // One accounted reserved lifecycle slot remains available even when
        // ordinary admission is full. Failed SDK finalization retains that slot.
        let capacity = if handler.lifecycle {
            1
        } else {
            self.capacity.get()
        };
        if occupied >= capacity && !handler.resident {
            return Err(ServingError::Capacity);
        }
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (handler.prepare)(payload, grpc)
        }))
        .map_err(|_| ServingError::Panicked)??;
        let answer = encode_prepared_grant(prepared.grant().as_ref());
        let slot = Slot {
            lifecycle: handler.lifecycle,
            resident: handler.resident,
            #[cfg(feature = "streaming")]
            streams: prepared.streams(),
            state: Mutex::new(SlotState {
                prepared: Some(prepared),
                finished: false,
                waiters: Vec::new(),
            }),
        };
        self.prepared.insert(invocation, Arc::new(slot));
        Ok(answer)
    }

    #[cfg(feature = "streaming")]
    pub fn streams(&self, invocation: u64) -> Result<WireStreams, ServingError> {
        let slot = self
            .prepared
            .get(&invocation)
            .ok_or(ServingError::Missing { invocation })?;
        if self.closed || (self.ordinary_closed && !slot.lifecycle) || slot.is_finished() {
            return Err(ServingError::Closing);
        }
        slot.streams.clone().ok_or(ServingError::StreamPhase)
    }

    pub async fn initialize(&mut self, invocation: u64, context: &C) -> Result<(), ServingError> {
        if self.closed {
            return Err(ServingError::Closing);
        }
        let slot = self
            .prepared
            .get(&invocation)
            .ok_or(ServingError::Missing { invocation })?;
        if self.ordinary_closed && !slot.lifecycle {
            return Err(ServingError::Closing);
        }
        slot.initialize(invocation, context).await
    }

    pub async fn attempt(&mut self, invocation: u64, context: &C) -> Result<Vec<u8>, ServingError> {
        if self.closed {
            return Err(ServingError::Closing);
        }
        let slot = self
            .prepared
            .get(&invocation)
            .ok_or(ServingError::Missing { invocation })?;
        if self.ordinary_closed && !slot.lifecycle {
            return Err(ServingError::Closing);
        }
        slot.attempt(invocation, context, None).await
    }

    pub async fn finalize(&mut self, invocation: u64) -> Result<(), ServingError> {
        if let Some(slot) = self.prepared.get(&invocation) {
            slot.finalize().await?;
        }
        self.prepared.remove(&invocation);
        Ok(())
    }

    fn traffic(
        &self,
        command: WireCommand,
    ) -> Pin<Box<dyn Future<Output = WireAnswer> + Send + 'static>> {
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
                    WireCommand::StreamSend { epoch, payload, .. } => wire_answer(
                        guarded(async { streams.send(epoch, &payload).await.map(|()| Vec::new()) })
                            .await,
                    ),
                    WireCommand::StreamFinish { epoch, .. } => {
                        wire_answer(streams.finish_input(epoch).map(|()| Vec::new()))
                    }
                    WireCommand::StreamReceive { epoch, .. } => {
                        match guarded(streams.receive(epoch)).await {
                            Ok(payload) => WireAnswer::Stream {
                                epoch,
                                transferred: streams.transferred(),
                                payload,
                            },
                            Err(error) => wire_answer(Err(error)),
                        }
                    }
                    _ => unreachable!("only stream traffic bypasses the phase loan"),
                }
            })
        }
    }

    pub fn begin(
        &mut self,
        command: WireCommand,
        context: Arc<C>,
    ) -> Pin<Box<dyn Future<Output = WireAnswer> + Send + 'static>> {
        if command.is_stream_traffic() {
            return self.traffic(command);
        }
        let slot = match &command {
            WireCommand::Initialize { invocation, .. }
            | WireCommand::Attempt { invocation, .. }
            | WireCommand::StreamAttempt { invocation, .. }
            | WireCommand::Finalize { invocation } => self.prepared.get(invocation).cloned(),
            WireCommand::Prepare { .. } | WireCommand::PrepareGrpc { .. } => None,
            WireCommand::StreamSend { .. }
            | WireCommand::StreamFinish { .. }
            | WireCommand::StreamReceive { .. } => {
                unreachable!("stream traffic already dispatched")
            }
        };
        let command = match command {
            WireCommand::PrepareGrpc {
                invocation,
                operation,
                payload,
            } => {
                let result = self.prepare_codec(invocation, &operation, payload, true);
                return Box::pin(async move { wire_answer(result) });
            }
            WireCommand::Prepare {
                invocation,
                operation,
                payload,
            } => {
                let result = self.prepare(invocation, &operation, payload);
                return Box::pin(async move { wire_answer(result) });
            }
            command => command,
        };
        let refused = self.closed
            || (self.ordinary_closed && slot.as_ref().is_some_and(|slot| !slot.lifecycle));
        if refused
            && matches!(
                command,
                WireCommand::Initialize { .. }
                    | WireCommand::Attempt { .. }
                    | WireCommand::StreamAttempt { .. }
            )
        {
            return Box::pin(async { wire_answer(Err(ServingError::Closing)) });
        }
        Box::pin(async move {
            let result = match command {
                WireCommand::Initialize { invocation, .. } => match slot {
                    Some(slot) => slot
                        .initialize(invocation, &context)
                        .await
                        .map(|()| Vec::new()),
                    None => Err(ServingError::Missing { invocation }),
                },
                WireCommand::Attempt { invocation, .. } => match slot {
                    Some(slot) => slot.attempt(invocation, &context, None).await,
                    None => Err(ServingError::Missing { invocation }),
                },
                WireCommand::StreamAttempt {
                    invocation, epoch, ..
                } => match slot {
                    Some(slot) => slot.attempt(invocation, &context, Some(epoch)).await,
                    None => Err(ServingError::Missing { invocation }),
                },
                WireCommand::StreamSend { .. }
                | WireCommand::StreamFinish { .. }
                | WireCommand::StreamReceive { .. } => {
                    unreachable!("stream traffic already dispatched")
                }
                WireCommand::Finalize { .. } => match slot {
                    Some(slot) => slot.finalize().await.map(|()| Vec::new()),
                    None => Ok(Vec::new()),
                },
                WireCommand::Prepare { .. } | WireCommand::PrepareGrpc { .. } => {
                    unreachable!("preparation is synchronous")
                }
            };
            wire_answer(result)
        })
    }

    pub fn close_ordinary(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + 'static>> {
        self.ordinary_closed = true;
        let slots: Vec<_> = self
            .prepared
            .values()
            .filter(|slot| !slot.lifecycle)
            .cloned()
            .collect();
        #[cfg(feature = "streaming")]
        for slot in &slots {
            if let Some(streams) = &slot.streams {
                streams.close();
            }
        }
        Box::pin(async move {
            for slot in slots {
                slot.finalize().await?;
            }
            Ok(())
        })
    }

    pub fn release_ordinary(&mut self) -> Result<impl Send + use<C>, ServingError> {
        if !self.ordinary_closed
            || self
                .prepared
                .values()
                .any(|slot| !slot.lifecycle && !slot.is_finished())
        {
            return Err(ServingError::Unsettled);
        }
        // Hand Off Retired Owners So The Caller Can Drop Them Outside Its Lock
        let retired_slots = self
            .prepared
            .extract_if(.., |_, slot| !slot.lifecycle)
            .collect::<Vec<_>>();
        let retired_handlers = self
            .handlers
            .extract_if(.., |_, handler| !handler.lifecycle)
            .collect::<Vec<_>>();
        Ok((retired_slots, retired_handlers))
    }

    pub async fn settle_ordinary(&mut self) -> Result<(), ServingError> {
        self.close_ordinary().await?;
        let retired = self.release_ordinary()?;
        drop(retired);
        Ok(())
    }

    pub fn close(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), ServingError>> + Send + 'static>> {
        self.closed = true;
        let slots: Vec<_> = self.prepared.values().cloned().collect();
        #[cfg(feature = "streaming")]
        for slot in &slots {
            if let Some(streams) = &slot.streams {
                streams.close();
            }
        }
        Box::pin(async move {
            for slot in slots {
                slot.finalize().await?;
            }
            Ok(())
        })
    }

    pub fn release(&mut self) -> Result<impl Send + use<C>, ServingError> {
        if !self.closed || self.prepared.values().any(|slot| !slot.is_finished()) {
            return Err(ServingError::Unsettled);
        }
        Ok((
            std::mem::take(&mut self.prepared),
            std::mem::take(&mut self.handlers),
        ))
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

    pub async fn answer(&mut self, command: WireCommand, context: &C) -> WireAnswer {
        if command.is_stream_traffic() {
            return self.traffic(command).await;
        }
        if let WireCommand::StreamAttempt {
            invocation, epoch, ..
        } = command
        {
            if self.closed
                || (self.ordinary_closed
                    && self
                        .prepared
                        .get(&invocation)
                        .is_some_and(|slot| !slot.lifecycle))
            {
                return wire_answer(Err(ServingError::Closing));
            }
            let Some(slot) = self.prepared.get(&invocation) else {
                return wire_answer(Err(ServingError::Missing { invocation }));
            };
            return wire_answer(slot.attempt(invocation, context, Some(epoch)).await);
        }
        let result = match command {
            WireCommand::PrepareGrpc {
                invocation,
                operation,
                payload,
            } => self.prepare_codec(invocation, &operation, payload, true),
            WireCommand::Prepare {
                invocation,
                operation,
                payload,
            } => self.prepare(invocation, &operation, payload),
            WireCommand::Initialize { invocation, .. } => self
                .initialize(invocation, context)
                .await
                .map(|()| Vec::new()),
            WireCommand::Attempt { invocation, .. } => self.attempt(invocation, context).await,
            WireCommand::StreamAttempt { .. }
            | WireCommand::StreamSend { .. }
            | WireCommand::StreamFinish { .. }
            | WireCommand::StreamReceive { .. } => {
                unreachable!("stream command already dispatched")
            }
            WireCommand::Finalize { invocation } => {
                self.finalize(invocation).await.map(|()| Vec::new())
            }
        };
        wire_answer(result)
    }
}

impl<C> Slot<C> {
    fn is_finished(&self) -> bool {
        self.state.lock().expect("invocation state lock").finished
    }

    async fn borrow(self: &Arc<Self>) -> Option<Loan<C>> {
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
            if !state
                .waiters
                .iter()
                .any(|entry| entry.ptr_eq(&Arc::downgrade(&waiter)))
            {
                state.waiters.push(Arc::downgrade(&waiter));
            }
            Poll::Pending
        })
        .await
    }
}

impl<C: Send + Sync + 'static> Slot<C> {
    async fn initialize(
        self: &Arc<Self>,
        invocation: u64,
        context: &C,
    ) -> Result<(), ServingError> {
        let mut loan = self
            .borrow()
            .await
            .ok_or(ServingError::Missing { invocation })?;
        loan.prepared
            .as_mut()
            .expect("loaned invocation state")
            .initialize(context)
            .await
    }

    async fn attempt(
        self: &Arc<Self>,
        invocation: u64,
        context: &C,
        epoch: Option<u64>,
    ) -> Result<Vec<u8>, ServingError> {
        let mut loan = self
            .borrow()
            .await
            .ok_or(ServingError::Missing { invocation })?;
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
        let result = loan
            .prepared
            .as_mut()
            .expect("loaned invocation state")
            .attempt(context)
            .await;
        #[cfg(feature = "streaming")]
        if let Some(attempt) = attempt {
            attempt.end();
        }
        result
    }

    async fn finalize(self: &Arc<Self>) -> Result<(), ServingError> {
        #[cfg(feature = "streaming")]
        if let Some(streams) = &self.streams {
            streams.close();
        }
        let Some(mut loan) = self.borrow().await else {
            return Ok(());
        };
        loan.prepared
            .as_mut()
            .expect("loaned invocation state")
            .finalize()
            .await?;
        // Drop Physical State Before Releasing Its Capacity
        loan.finished = true;
        let prepared = loan.prepared.take();
        drop(prepared);
        Ok(())
    }
}

impl<C> Drop for Loan<C> {
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
        matches!(
            self,
            Self::StreamSend { .. } | Self::StreamFinish { .. } | Self::StreamReceive { .. }
        )
    }
}

impl ServingError {
    pub fn classification(&self) -> Failure {
        match self {
            Self::Handler { failure, .. } => *failure,
            Self::Panicked => Failure::Ambiguous,
            Self::Codec(_)
            | Self::Declaration(_)
            | Self::Lifecycle(_)
            | Self::Contract(_)
            | Self::Catalog
            | Self::Streaming { .. } => Failure::Invalid,
            _ => Failure::Permanent,
        }
    }
}

impl<C: Send + Sync + 'static, H: Send + Sync + 'static> Visitor for Serves<H, C> {
    type Entry = WireBinding<H, C>;
}

#[cfg(not(feature = "streaming"))]
impl<T, R, C> Member<T> for Serves<R, C>
where
    C: Send + Sync + 'static,
    R: Send + Sync + 'static,
    T: for<'a> Operation<Resources = R, Context<'a> = &'a C, Endpoints = ()>,
    T::Declaration: WireOperation,
    <T::Declaration as OperationDeclaration>::Request: Message + Described,
    <T::Declaration as OperationDeclaration>::Response: Message + Described,
{
    const ENTRY: WireBinding<R, C> = WireBinding {
        install: WireTable::<C>::bind_operation::<T>,
    };
}

#[cfg(feature = "streaming")]
impl<T, R, C> Member<T> for Serves<R, C>
where
    C: Send + Sync + 'static,
    R: Send + Sync + 'static,
    T: for<'a> Operation<Resources = R, Context<'a> = &'a C>,
    T::Endpoints: WireEndpoints,
    T::Declaration: WireOperation,
    <T::Declaration as OperationDeclaration>::Request: Message + Described,
    <T::Declaration as OperationDeclaration>::Response: Message + Described,
{
    const ENTRY: WireBinding<R, C> = WireBinding {
        install: WireTable::<C>::bind_catalog_operation::<T>,
    };
}

/* === Public Functions === */
// A Prepare answer is empty when the request needs no grant, else the grant as JSON.
pub fn encode_prepared_grant(grant: Option<&crate::plugin::Grant>) -> Vec<u8> {
    grant.map_or_else(Vec::new, |grant| {
        serde_json::to_vec(grant).expect("a grant serializes to JSON")
    })
}

pub fn decode_prepared_grant(
    payload: &[u8],
) -> Result<Option<crate::plugin::Grant>, serde_json::Error> {
    if payload.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(payload).map(Some)
}

/* === Private Functions === */
fn wire_answer(result: Result<Vec<u8>, ServingError>) -> WireAnswer {
    match result {
        Ok(payload) => WireAnswer::Ready { payload },
        Err(error) => WireAnswer::Failed {
            failure: error.classification(),
            message: error.to_string(),
        },
    }
}

async fn guarded<T>(
    future: impl Future<Output = Result<T, ServingError>>,
) -> Result<T, ServingError> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(result) => result,
            Err(_) => std::task::Poll::Ready(Err(ServingError::Panicked)),
        }
    })
    .await
}

/* === Tests === */
