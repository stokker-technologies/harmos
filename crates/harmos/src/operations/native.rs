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
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
// Local Imports
use super::children::{CHILD_REQUESTS, ChildRequest, Children};
use super::{Context, Operation, OperationManager};
use crate::catalog::{Catalog, Member, Visitor};
use crate::operations::{
    Definition, Failure, OperationDeclaration, OperationPolicy, RetryPolicy, WireAnswer,
    WireCommand, WireOperation,
};
use crate::schema::Described;
use crate::services::{NativeEndpoint, SidecarError};
use crate::{Error, Identifier, Message, Result, operations::Lane};

/* === Constants === */
const MAX_NATIVE_OPERATIONS: usize = 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Native {
    endpoint: Arc<NativeEndpoint>,
}

struct NativeState {
    grpc: bool,
    initialized: bool,
    payload: Option<Vec<u8>>,
    invocation: Option<u64>,
    operation: Identifier,
    sdk: Option<harmos_contract::operations::lifecycle::Action>,
    acknowledged: bool,
    _capacity: Option<OwnedSemaphorePermit>,
}

pub struct NativeOperation<O: OperationDeclaration, E = ()> {
    native: Arc<Native>,
    state: Option<NativeState>,
    epoch: u64,
    transferred: AtomicBool,
    _operation: PhantomData<fn() -> (O, E)>,
}

pub struct NativeBindings;

struct EncodedOperation<E = ()> {
    binding: Arc<Binding>,
    state: Option<NativeState>,
    epoch: u64,
    transferred: AtomicBool,
    endpoints: PhantomData<fn() -> E>,
}

pub(crate) struct Encoded<E = ()>(PhantomData<fn() -> E>);

#[derive(Clone)]
pub(crate) struct Opaque(Vec<u8>);

// Bytes And A Pre-Owned Permit Stay Together Through Original Preparation.
pub(crate) struct EncodedRequest {
    grpc: bool,
    payload: Vec<u8>,
    capacity: Option<OwnedSemaphorePermit>,
}

pub(super) struct Binding {
    native: Arc<Native>,
    selection: Selection,
    capacity: Arc<Semaphore>,
}

struct Project<'a, 'b> {
    bindings: &'a mut super::Bindings<'b>,
    lane: &'static str,
    resources: Arc<Native>,
}

/* === Enums === */
enum Phase {
    Initialize,
    Attempt,
    Stream(u64),
}

enum Selection {
    Provided(usize),
    Sdk(Definition),
}

#[derive(Debug, thiserror::Error)]
pub enum NativeError {
    #[error(transparent)]
    Stream(#[from] super::RemoteStreamError),
    #[error("native provider retention failed: {0}")]
    Provider(#[source] crate::Error),
    #[error("the selected native generation is unavailable")]
    Unavailable,
    #[error("native operation capacity is retained")]
    Capacity,
    #[error("native attempt context entropy is unavailable: {0}")]
    Entropy(#[from] getrandom::Error),
    #[error("operation `{operation}` differs from the selected native catalog")]
    Catalog { operation: String },
    #[error(transparent)]
    Declaration(#[from] harmos_contract::Error),
    #[error("native operation transport failed: {0}")]
    Transport(#[from] SidecarError),
    #[error("native operation response codec failed: {0}")]
    Payload(#[from] crate::MessageError),
    #[error("native operation preparation failed: {message}")]
    Preparation { failure: Failure, message: String },
    #[error("native operation failed: {message}")]
    Remote { failure: Failure, message: String },
}

/* === Traits === */

/* === Implementations === */
impl NativeError {
    pub(crate) fn classification(&self) -> Failure {
        classify(self)
    }
}

impl super::WireVisitor for Project<'_, '_> {
    type Entry = crate::Result<()>;
    fn wire<O>(self) -> Self::Entry
    where
        O: WireOperation,
        O::Request: Message + Described,
        O::Response: Message + Described,
    {
        self.bindings
            .register_owned_wire_at::<NativeOperation<O, ()>>(self.lane, self.resources)
    }

    fn upload<O>(self) -> Self::Entry
    where
        O: WireOperation + crate::operations::InputDeclaration,
        O::Request: Message + Described,
        O::Response: Message + Described,
        O::Item: Message + crate::MemorySize,
    {
        self.bindings
            .register_owned_wire_at::<NativeOperation<O, super::StreamInput<O::Item>>>(
                self.lane,
                self.resources,
            )
    }

    fn download<O>(self) -> Self::Entry
    where
        O: WireOperation + crate::operations::OutputDeclaration,
        O::Request: Message + Described,
        O::Response: Message + Described,
        O::Item: Message + crate::MemorySize,
    {
        self.bindings
            .register_owned_wire_at::<NativeOperation<O, super::StreamOutput<O::Item>>>(
                self.lane,
                self.resources,
            )
    }

    fn exchange<O>(self) -> Self::Entry
    where
        O: WireOperation
            + crate::operations::InputDeclaration
            + crate::operations::OutputDeclaration,
        O::Request: Message + Described,
        O::Response: Message + Described,
        <O as crate::operations::InputDeclaration>::Item: Message + crate::MemorySize,
        <O as crate::operations::OutputDeclaration>::Item: Message + crate::MemorySize,
    {
        self.bindings.register_owned_wire_at::<NativeOperation<
            O,
            (
                super::StreamInput<<O as crate::operations::InputDeclaration>::Item>,
                super::StreamOutput<<O as crate::operations::OutputDeclaration>::Item>,
            ),
        >>(self.lane, self.resources)
    }
}

impl super::StreamPeer for Native {
    type Error = NativeError;
    async fn stream_phase(
        &self,
        command: WireCommand,
    ) -> std::result::Result<WireAnswer, Self::Error> {
        self.endpoint
            .phase(command)
            .await
            .map_err(NativeError::from)
    }
}

#[cfg(feature = "grpc")]
impl crate::Protobuf for EncodedRequest {
    fn encode_to_vec(&self) -> Vec<u8> {
        self.payload.clone()
    }

    fn encoded_len(&self) -> usize {
        self.payload.len()
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
        std::mem::size_of::<Self>()
            .checked_add(bytes.len())
            .ok_or_else(|| crate::MessageError::borrowed("forwarded request size overflows"))
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
        Ok(Self {
            payload: bytes.to_vec(),
            grpc: true,
            capacity: None,
        })
    }
}
#[cfg(feature = "grpc")]
impl crate::MemorySize for EncodedRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.payload.capacity())
    }
}

impl Message for Opaque {
    fn encode_to_vec(&self) -> Vec<u8> {
        self.0.clone()
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.0);
    }

    fn encoded_len(&self) -> usize {
        self.0.len()
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
        std::mem::size_of::<Self>()
            .checked_add(bytes.len())
            .ok_or_else(|| crate::MessageError::borrowed("forwarded payload size overflows"))
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
        Ok(Self(bytes.to_vec()))
    }
}
impl crate::MemorySize for Opaque {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.0.capacity())
    }
}
#[cfg(feature = "grpc")]
impl crate::Protobuf for Opaque {
    fn encode_to_vec(&self) -> Vec<u8> {
        Message::encode_to_vec(self)
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        Message::encode_into(self, bytes);
    }

    fn encoded_len(&self) -> usize {
        Message::encoded_len(self)
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
        <Self as Message>::decoding_memory(bytes)
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
        <Self as Message>::decode(bytes)
    }
}

impl Native {
    pub fn scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.endpoint.scope()
    }

    pub(crate) fn initialization(&self) -> tokio::sync::watch::Receiver<bool> {
        self.endpoint.initialized()
    }

    pub(crate) fn new(endpoint: Arc<NativeEndpoint>) -> Self {
        Self { endpoint }
    }

    pub fn stage_sdk(
        &self,
        operations: &OperationManager,
        lane: impl Lane,
    ) -> Result<super::PluginSdkRegistration> {
        let scope = self.scope().ok_or(Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "native endpoint has no original plugin generation",
        })?;
        let pair = scope.lifecycle().ok_or(Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "generation has no frozen SDK pair",
        })?;
        let native = Arc::new(Self {
            endpoint: Arc::clone(&self.endpoint),
        });
        let capacity = Arc::new(Semaphore::new(1));
        let entries = pair
            .iter()
            .map(|descriptor| {
                let handler = Arc::new(Binding {
                    native: Arc::clone(&native),
                    selection: Selection::Sdk(descriptor.contract().operation().clone()),
                    capacity: Arc::clone(&capacity),
                });
                let suffix = match descriptor.action() {
                    harmos_contract::operations::lifecycle::Action::Initialize => "initialize",
                    harmos_contract::operations::lifecycle::Action::Shutdown => "shutdown",
                };
                let mut definition = descriptor.contract().operation().clone();
                definition.id = crate::Identifier::new(format!("{}.sdk.{suffix}", scope.owner()))
                    .map_err(|source| Error::OperationDeclaration {
                    source: harmos_contract::Error::from(source),
                })?;
                let mut entry = operations.binding_owned_definition::<EncodedOperation>(
                    scope.owner(),
                    lane.name(),
                    Arc::clone(&handler),
                    definition,
                    Some(dispatch),
                    Some(scope.clone()),
                )?;
                let selected = Arc::get_mut(&mut entry).expect("new SDK entry has one owner");
                selected.native = Some(handler);
                selected.contract = Some(descriptor.contract().clone());
                Ok(entry)
            })
            .collect::<Result<Vec<_>>>()?;
        let entries: [Arc<super::manager::Entry>; 2] =
            entries.try_into().map_err(|_| Error::InvalidInput {
                field: "SDK lifecycle",
                reason: "expected exactly two entries",
            })?;
        super::PluginSdkRegistration::new(
            operations.clone(),
            scope,
            entries,
            crate::plugin::PluginKind::Sidecar,
        )
    }
}

impl<O, E> Operation for NativeOperation<O, E>
where
    O: OperationDeclaration,
    E: super::StreamEndpoints,
    O::Request: Message,
    O::Response: Message,
{
    type Declaration = O;
    type Resources = Native;
    type Error = NativeError;
    type Endpoints = E;
    type Context<'a> = Context<'a>;

    fn new(native: Arc<Native>) -> std::result::Result<Self, NativeError> {
        Ok(Self {
            native,
            state: None,
            epoch: 0,
            transferred: AtomicBool::new(false),
            _operation: PhantomData,
        })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut O::Request,
        endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<O::Response, NativeError> {
        if self.state.is_none() {
            let native = &self.native;

            let definition = Definition::of::<O>()?;
            native.validate_definition(&definition)?;
            let capacity = native
                .endpoint
                .reserve_capacity()
                .ok_or(NativeError::Capacity)?;
            let payload = request.encode_to_vec();
            let state = native.prepare_encoded(&definition, payload, Some(capacity))?;

            self.state = Some(state);
        }
        let state = self.state.as_mut().expect("original remote state");
        if !state.initialized {
            self.native.initialize_encoded(state, &context).await?;
            state.initialized = true;
        }

        let payload = if E::item_types() == (None, None) {
            self.native.attempt_encoded(state, &context).await?
        } else {
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or(super::RemoteStreamError::Protocol(
                    "stream attempt counter exhausted",
                ))?;
            let running = self
                .native
                .phase_encoded(state, &context, Phase::Stream(self.epoch));
            let answer = super::remote::attempt(
                &*self.native,
                endpoints,
                state.invocation.expect("original prepared invocation"),
                self.epoch,
                &self.transferred,
                running,
            )
            .await?;
            let payload = ready(answer)?;
            state.acknowledged = true;
            payload
        };
        O::Response::decode(payload.as_slice()).map_err(NativeError::Payload)
    }

    async fn finalize(&mut self) -> std::result::Result<(), NativeError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        self.native.finalize_encoded(state).await
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }

    fn classify(error: &NativeError) -> Failure {
        classify(error)
    }
}

impl Visitor for NativeBindings {
    type Entry = super::Binding<Native>;
}

impl<O> Member<O> for NativeBindings
where
    O: WireOperation,
    O::Request: Message + Described,
    O::Response: Message + Described,
{
    const ENTRY: super::Binding<Native> = super::Binding {
        register: register::<O>,
    };
}

impl Native {
    fn prepare_encoded(
        &self,
        definition: &Definition,
        payload: Vec<u8>,
        capacity: Option<OwnedSemaphorePermit>,
    ) -> std::result::Result<NativeState, NativeError> {
        self.validate_definition(definition)?;
        if payload.capacity() > harmos_sidecar::Frame::LIMIT {
            let source = SidecarError::Refused {
                code: harmos_sidecar::Frame::OVERSIZED,
                message: "operation request exceeds the native frame limit".to_owned(),
            };
            return Err(source.into());
        }
        let state = NativeState {
            grpc: false,
            initialized: false,
            payload: Some(payload),
            invocation: None,
            operation: definition.id.clone(),
            sdk: None,
            acknowledged: false,
            _capacity: capacity,
        };
        Ok(state)
    }

    fn validate_definition(&self, definition: &Definition) -> std::result::Result<(), NativeError> {
        if !self.endpoint.accepts() {
            return Err(NativeError::Unavailable);
        }
        if !self.endpoint.definitions().contains(definition) {
            return Err(NativeError::Catalog {
                operation: definition.id.to_string(),
            });
        }
        Ok(())
    }

    async fn initialize_encoded<'ctx>(
        &self,
        state: &mut NativeState,
        context: &Context<'ctx>,
    ) -> std::result::Result<(), NativeError> {
        let dependent = context
            .scope
            .original_control()
            .upgrade()
            .ok_or(NativeError::Unavailable)?;
        self.endpoint
            .retain_provider(&dependent, context.retirement)
            .map_err(NativeError::Provider)?;
        if state.invocation.is_none() {
            let invocation = context.id().value();
            // Record Ownership Before Sending: A Lost Ack May Still Own Remote State
            state.invocation = Some(invocation);
            let payload = state.payload.take().expect("one inert request is owned");
            let command = if state.grpc {
                WireCommand::PrepareGrpc {
                    invocation,
                    operation: state.operation.to_string(),
                    payload,
                }
            } else {
                WireCommand::Prepare {
                    invocation,
                    operation: state.operation.to_string(),
                    payload,
                }
            };
            let answer = self.endpoint.phase(command).await?;
            let answer = match answer {
                WireAnswer::Failed { failure, message } => {
                    return Err(NativeError::Preparation { failure, message });
                }
                answer => answer,
            };
            acknowledged(answer)?;
        }
        let answer = self
            .phase_encoded(state, context, Phase::Initialize)
            .await?;
        match answer {
            WireAnswer::Failed { failure, message } => {
                Err(NativeError::Preparation { failure, message })
            }
            answer => acknowledged(answer),
        }
    }

    async fn phase_encoded<'ctx>(
        &self,
        state: &NativeState,
        context: &Context<'ctx>,
        phase: Phase,
    ) -> std::result::Result<WireAnswer, NativeError> {
        let invocation = state.invocation.expect("prepared identity is retained");
        let mut token = [0; 32];
        getrandom::fill(&mut token)?;
        let (requests, mut incoming) = mpsc::channel::<ChildRequest>(CHILD_REQUESTS);
        let children = Children {
            context: token,
            requests,
        };
        let command = match phase {
            Phase::Initialize => WireCommand::Initialize {
                invocation,
                context: token,
            },
            Phase::Stream(epoch) => WireCommand::StreamAttempt {
                invocation,
                epoch,
                context: token,
                values: context.context().portable(),
            },
            Phase::Attempt => WireCommand::Attempt {
                invocation,
                context: token,
                values: context.context().portable(),
            },
        };
        let running = self.endpoint.attempt(command, children);
        tokio::pin!(running);
        let answer = loop {
            tokio::select! {
                biased;
                answer = &mut running => break answer?,
                Some(request) = incoming.recv() => {
                    if request.answer.is_closed() { continue; }
                    let child = context.submit_encoded_at(&request.operation, request.payload, request.submitted, request.values);
                    // A Cancelled Admission Reply Still Cancels Its Admitted Child
                    if let Err(Ok(child)) = request.answer.send(child) { child.cancel(); }
                }
            }
        };
        Ok(answer)
    }

    async fn attempt_encoded<'ctx>(
        &self,
        state: &mut NativeState,
        context: &Context<'ctx>,
    ) -> std::result::Result<Vec<u8>, NativeError> {
        let answer = self.phase_encoded(state, context, Phase::Attempt).await?;
        let payload = ready(answer)?;
        if state.sdk.is_some() && !payload.is_empty() {
            return Err(
                SidecarError::Malformed("expected empty SDK acknowledgement".to_owned()).into(),
            );
        }
        state.acknowledged = true;
        Ok(payload)
    }

    async fn finalize_encoded(
        &self,
        state: &mut NativeState,
    ) -> std::result::Result<(), NativeError> {
        let Some(invocation) = state.invocation else {
            return Ok(());
        };
        if !self.endpoint.accepts() {
            self.endpoint.settle().await?;
            return Ok(());
        }
        let command = WireCommand::Finalize { invocation };
        match self.endpoint.phase(command).await {
            Ok(answer) => {
                acknowledged(answer)?;
                if state.acknowledged
                    && state.sdk == Some(harmos_contract::operations::lifecycle::Action::Initialize)
                {
                    self.endpoint.acknowledge_initialize();
                }
                Ok(())
            }
            Err(SidecarError::Unavailable) => {
                self.endpoint.settle().await?;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }
}

impl<E: super::StreamEndpoints> OperationDeclaration for Encoded<E> {
    type Request = EncodedRequest;
    type Response = Opaque;
    fn endpoint_types() -> (Option<std::any::TypeId>, Option<std::any::TypeId>) {
        E::item_types()
    }
    // This Adapter Is Never Declared Or Registered By Its Static Identity.
    const ID: &'static str = "";
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::ZERO,
        attempt_timeout: Duration::ZERO,
        retries: RetryPolicy::Never,
    };
}

impl crate::operations::InputDeclaration for Encoded<super::StreamInput<Opaque>> {
    type Item = Opaque;
}
impl crate::operations::OutputDeclaration for Encoded<super::StreamOutput<Opaque>> {
    type Item = Opaque;
}
impl crate::operations::InputDeclaration
    for Encoded<(super::StreamInput<Opaque>, super::StreamOutput<Opaque>)>
{
    type Item = Opaque;
}
impl crate::operations::OutputDeclaration
    for Encoded<(super::StreamInput<Opaque>, super::StreamOutput<Opaque>)>
{
    type Item = Opaque;
}

impl<E: super::StreamEndpoints> Operation for EncodedOperation<E> {
    type Declaration = Encoded<E>;
    type Resources = Binding;
    type Error = NativeError;
    type Endpoints = E;
    type Context<'a> = Context<'a>;

    fn new(binding: Arc<Binding>) -> std::result::Result<Self, NativeError> {
        Ok(Self {
            binding,
            state: None,
            epoch: 0,
            transferred: AtomicBool::new(false),
            endpoints: PhantomData,
        })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut EncodedRequest,
        endpoints: &mut Self::Endpoints,
    ) -> std::result::Result<Opaque, NativeError> {
        if self.state.is_none() {
            let binding = &self.binding;

            let capacity = match request.capacity.take() {
                Some(capacity) => capacity,
                None => binding.reserve()?,
            };
            let mut state = match &binding.selection {
                Selection::Provided(index) => {
                    let definition = &binding.native.endpoint.definitions()[*index];
                    binding.native.prepare_encoded(
                        definition,
                        std::mem::take(&mut request.payload),
                        Some(capacity),
                    )
                }
                Selection::Sdk(definition) => {
                    if request.grpc {
                        return Err(NativeError::Catalog {
                            operation: definition.id.to_string(),
                        });
                    }
                    if !binding.native.endpoint.accepts() {
                        return Err(NativeError::Unavailable);
                    }
                    let matches = binding
                        .native
                        .scope()
                        .and_then(|scope| scope.lifecycle())
                        .is_some_and(|pair| {
                            pair.iter()
                                .any(|descriptor| descriptor.contract().operation() == definition)
                        });
                    if !matches || request.payload.capacity() > harmos_sidecar::Frame::LIMIT {
                        return Err(NativeError::Catalog {
                            operation: definition.id.to_string(),
                        });
                    }
                    let action = binding
                        .native
                        .scope()
                        .and_then(|scope| scope.lifecycle())
                        .and_then(|pair| {
                            pair.iter()
                                .find(|descriptor| descriptor.contract().operation() == definition)
                        })
                        .map(|descriptor| descriptor.action());
                    Ok(NativeState {
                        grpc: false,
                        initialized: false,
                        payload: Some(std::mem::take(&mut request.payload)),
                        invocation: None,
                        operation: definition.id.clone(),
                        sdk: action,
                        acknowledged: false,
                        _capacity: Some(capacity),
                    })
                }
            }?;

            state.grpc = request.grpc;
            self.state = Some(state);
        }
        let state = self.state.as_mut().expect("original remote state");
        if !state.initialized {
            self.binding
                .native
                .initialize_encoded(state, &context)
                .await?;
            state.initialized = true;
        }

        let native = &self.binding.native;
        let payload = if E::item_types() == (None, None) {
            native.attempt_encoded(state, &context).await?
        } else {
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or(super::RemoteStreamError::Protocol(
                    "stream attempt counter exhausted",
                ))?;
            let running = native.phase_encoded(state, &context, Phase::Stream(self.epoch));
            let answer = super::remote::attempt(
                &**native,
                endpoints,
                state.invocation.expect("original prepared invocation"),
                self.epoch,
                &self.transferred,
                running,
            )
            .await?;
            let payload = ready(answer)?;
            state.acknowledged = true;
            payload
        };
        Ok(Opaque(payload))
    }

    async fn finalize(&mut self) -> std::result::Result<(), NativeError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        self.binding.native.finalize_encoded(state).await
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }

    fn classify(error: &NativeError) -> Failure {
        classify(error)
    }
}

impl Binding {
    pub(super) async fn settle_observers(&self) -> Result<()> {
        self.native.endpoint.settle_observers().await
    }

    pub(super) fn observer_outcome(&self) -> Result<()> {
        self.native.endpoint.observer_outcome()
    }

    pub(super) fn capture_observer_controls(&self) -> Result<()> {
        self.native.endpoint.capture_observer_controls()
    }

    pub(super) fn reserve(&self) -> std::result::Result<OwnedSemaphorePermit, NativeError> {
        Arc::clone(&self.capacity)
            .try_acquire_owned()
            .map_err(|_| NativeError::Capacity)
    }
}

impl super::Bindings<'_> {
    pub fn register_native_catalog<C>(
        &mut self,
        lane: impl Lane,
        native: Arc<Native>,
        capacity: NonZeroUsize,
    ) -> Result<()>
    where
        C: Catalog<crate::operations::Declarations> + Catalog<NativeBindings>,
    {
        let scope = self.scope.ok_or(Error::InvalidInput {
            field: "native operation scope",
            reason: "native inventory requires stage_plugin",
        })?;
        validate_inventory(&native, scope)?;
        let previous = self.entries.len();
        self.project::<C, NativeBindings, Native>(lane, Arc::clone(&native))?;
        let projected = &self.entries[previous..];
        let exact = projected.iter().all(|entry| {
            entry.contract.as_ref().is_some_and(|contract| {
                native
                    .endpoint
                    .contracts()
                    .iter()
                    .filter(|original| *original == contract)
                    .count()
                    == 1
            })
        });
        if !exact {
            self.entries.truncate(previous);
            return Err(Error::InvalidInput {
                field: "native operation catalog",
                reason: "typed projection differs from the frozen native wire contracts",
            });
        }
        let capacity = match native.endpoint.bind_capacity(capacity) {
            Ok(capacity) => capacity,
            Err(error) => {
                self.entries.truncate(previous);
                return Err(error);
            }
        };
        for entry in &mut self.entries[previous..] {
            let index = native
                .endpoint
                .definitions()
                .iter()
                .position(|definition| definition == entry.declared())
                .expect("exact typed contract belongs to the validated native inventory");
            let handler = Arc::new(Binding {
                native: Arc::clone(&native),
                selection: Selection::Provided(index),
                capacity: Arc::clone(&capacity),
            });
            let entry = Arc::get_mut(entry).expect("new typed native entry has one owner");
            if entry
                .contract
                .as_ref()
                .is_some_and(|contract| contract.input().is_none() && contract.output().is_none())
            {
                let encoded = super::authored::factory::<EncodedOperation>(
                    self.operations,
                    Arc::clone(&handler),
                );
                entry.native_binding = Some(Arc::new(encoded));
                entry.wire = Some(dispatch);
            } else {
                // A Unary Child Call Cannot Supply The Declared Stream Endpoints.
                entry.wire = None;
            }
            entry.native = Some(handler);
        }
        Ok(())
    }

    pub fn register_native(
        &mut self,
        lane: impl Lane,
        native: Arc<Native>,
        capacity: NonZeroUsize,
    ) -> Result<()> {
        let scope = self.scope.ok_or(Error::InvalidInput {
            field: "native operation scope",
            reason: "native inventory requires stage_plugin",
        })?;
        let entries = bindings(self.operations, lane.name(), native, capacity, scope)?;
        if let Some(entry) = entries.iter().find(|entry| {
            self.entries
                .iter()
                .any(|previous| previous.id() == entry.id())
        }) {
            return Err(Error::AlreadyRegistered {
                domain: "operation",
                name: entry.id().to_owned(),
            });
        }
        self.entries.extend(entries);
        Ok(())
    }
}

/* === Public Functions === */
pub(super) fn bindings(
    operations: &super::OperationManager,
    lane: &'static str,
    native: Arc<Native>,
    capacity: NonZeroUsize,
    scope: &crate::plugin::PluginScope,
) -> Result<Vec<Arc<super::manager::Entry>>> {
    validate_inventory(&native, scope)?;
    let capacity = native.endpoint.bind_capacity(capacity)?;
    let definitions = native.endpoint.definitions();
    definitions
        .iter()
        .enumerate()
        .map(|(index, definition)| {
            let handler = Arc::new(Binding {
                native: Arc::clone(&native),
                selection: Selection::Provided(index),
                capacity: Arc::clone(&capacity),
            });
            let mut contracts = native
                .endpoint
                .contracts()
                .iter()
                .filter(|contract| contract.operation().id == definition.id);
            let contract = contracts
                .next()
                .filter(|contract| contract.operation() == definition)
                .cloned();
            let exact_contract = if contracts.next().is_none() {
                contract
            } else {
                None
            };
            let shape = exact_contract.as_ref().map_or((false, false), |contract| {
                (contract.input().is_some(), contract.output().is_some())
            });
            let mut entry = match shape {
                (false, false) => {
                    encoded_binding::<()>(operations, lane, &handler, definition, scope)
                }
                (true, false) => encoded_binding::<super::StreamInput<Opaque>>(
                    operations, lane, &handler, definition, scope,
                ),
                (false, true) => encoded_binding::<super::StreamOutput<Opaque>>(
                    operations, lane, &handler, definition, scope,
                ),
                (true, true) => encoded_binding::<(
                    super::StreamInput<Opaque>,
                    super::StreamOutput<Opaque>,
                )>(operations, lane, &handler, definition, scope),
            }?;
            let entry_mut = Arc::get_mut(&mut entry).expect("new native entry has one owner");
            entry_mut.native = Some(handler);
            entry_mut.observer_stop = scope
                .observer_controls()
                .is_some_and(|pair| exact_contract.as_ref() == Some(&pair[1]));
            #[cfg(feature = "grpc")]
            if exact_contract
                .as_ref()
                .is_some_and(|contract| contract.grpc().is_some())
            {
                entry_mut.grpc = Some(match shape {
                    (false, false) => crate::grpc::server::unary::<Encoded>,
                    (true, false) => {
                        crate::grpc::server::upload::<Encoded<super::StreamInput<Opaque>>>
                    }
                    (false, true) => {
                        crate::grpc::server::download::<Encoded<super::StreamOutput<Opaque>>>
                    }
                    (true, true) => {
                        crate::grpc::server::exchange::<
                            Encoded<(super::StreamInput<Opaque>, super::StreamOutput<Opaque>)>,
                        >
                    }
                });
            }
            entry_mut.contract = exact_contract;
            Ok(entry)
        })
        .collect()
}

pub(super) fn dispatch(
    operations: &OperationManager,
    mut submission: super::manager::Submission,
    source: super::caller::Source,
    parent: Option<super::wire::Parent<'_>>,
    input: super::wire::EncodedInput,
) -> Result<super::EncodedInvocation> {
    submission.preparation = input.preparation;
    let binding = submission
        .entry
        .native
        .as_ref()
        .ok_or(Error::InvalidInput {
            field: "native operation",
            reason: "encoded native admission requires its original binding",
        })?;
    let failure = |source: NativeError| Error::OperationFailed {
        operation: submission.entry.id().to_owned(),
        source: Arc::new(source),
    };
    let capacity = match input.native_capacity {
        Some(capacity) => capacity,
        None => binding.reserve().map_err(failure)?,
    };
    if input.payload.capacity() > harmos_sidecar::Frame::LIMIT {
        return Err(failure(NativeError::Transport(SidecarError::Refused {
            code: harmos_sidecar::Frame::OVERSIZED,
            message: "operation request exceeds the native frame limit".to_owned(),
        })));
    }
    let request = EncodedRequest {
        grpc: false,
        payload: input.payload,
        capacity: Some(capacity),
    };
    // Bytes Are Already Encoded; The Selected Remote WireTable Owns Decoding.
    let submitted = submission.submitted;
    let timeout = submission.entry.declared().policy.timeout;
    let target = Arc::downgrade(&submission.entry);
    super::wire::budget(timeout, submitted)?;
    let (scope, ancestors) = parent.map_or((None, &[][..]), |parent| {
        (Some(parent.scope), parent.ancestors)
    });
    let invocation = operations.admit_entry::<Encoded, ()>(
        (request.into(), ()),
        scope,
        ancestors,
        &source,
        submission,
    )?;
    let id = invocation.id();
    let generation = invocation.generation();
    let control = Arc::clone(&invocation.control);
    let outcome = Box::pin(async move {
        let response = invocation.await?;
        super::wire::available(&target, &source)?;
        super::wire::budget(timeout, submitted)?;
        // No Cache Opt-In Exists For An Opaque Inspected Request.
        let payload = Arc::try_unwrap(response)
            .unwrap_or_else(|response| (*response).clone())
            .0;
        super::wire::available(&target, &source)?;
        super::wire::budget(timeout, submitted)?;
        Ok(payload)
    });
    Ok(super::EncodedInvocation {
        id,
        generation,
        outcome,
        control,
    })
}

/* === Private Functions === */
fn encoded_binding<E: super::StreamEndpoints>(
    operations: &OperationManager,
    lane: &'static str,
    handler: &Arc<Binding>,
    definition: &Definition,
    scope: &crate::plugin::PluginScope,
) -> Result<Arc<super::manager::Entry>> {
    operations.binding_owned_definition::<EncodedOperation<E>>(
        scope.owner(),
        lane,
        Arc::clone(handler),
        definition.clone(),
        (E::item_types() == (None, None)).then_some(dispatch),
        Some(scope.clone()),
    )
}

fn register<O>(
    bindings: &mut super::Bindings<'_>,
    lane: &'static str,
    native: Arc<Native>,
) -> Result<()>
where
    O: WireOperation,
    O::Request: Message + Described,
    O::Response: Message + Described,
{
    if let Some(projected) = O::visit_wire(Project {
        bindings,
        lane,
        resources: Arc::clone(&native),
    }) {
        projected
    } else {
        bindings.register_owned_wire_at::<NativeOperation<O>>(lane, native)
    }
}

fn validate_inventory(native: &Native, scope: &crate::plugin::PluginScope) -> Result<()> {
    let selected = native.scope().ok_or(Error::InvalidInput {
        field: "native operation scope",
        reason: "native endpoint has no admitted plugin generation",
    })?;
    if !selected.same_activation(scope) {
        return Err(Error::InvalidInput {
            field: "native operation scope",
            reason: "native endpoint belongs to a different plugin generation",
        });
    }
    if !native.endpoint.accepts() {
        return Err(Error::Closing {
            domain: "native operations",
        });
    }
    let definitions = native.endpoint.definitions();
    if definitions.len() > MAX_NATIVE_OPERATIONS {
        return Err(Error::ResourceExhausted {
            resource: "native operation inventory",
        });
    }
    let mut bytes = 0usize;
    for definition in definitions {
        definition
            .validate()
            .map_err(|source| Error::OperationDeclaration { source })?;
        bytes = bytes
            .checked_add(std::mem::size_of::<super::manager::Entry>())
            .and_then(|bytes| bytes.checked_add(definition.id.as_str().len()))
            .and_then(|bytes| bytes.checked_add(definition.description.len()))
            .filter(|bytes| *bytes <= harmos_sidecar::Frame::LIMIT)
            .ok_or(Error::ResourceExhausted {
                resource: "native operation inventory",
            })?;
    }
    // Validate The Complete Frozen Inventory Before Allocating Bindings.
    scope.validate_operations(definitions.iter())?;
    scope.validate_wire_contracts(native.endpoint.contracts().iter())?;
    Ok(())
}

fn classify(error: &NativeError) -> Failure {
    match error {
        NativeError::Stream(source) => source.classification(),
        NativeError::Remote { failure, .. } => *failure,
        NativeError::Preparation { failure, .. } => match failure {
            Failure::Invalid | Failure::Unauthorized | Failure::Cancelled => *failure,
            _ => Failure::Permanent,
        },
        NativeError::Transport(SidecarError::Unavailable | SidecarError::Deadline) => {
            Failure::Ambiguous
        }
        NativeError::Payload(_)
        | NativeError::Declaration(_)
        | NativeError::Catalog { .. }
        | NativeError::Transport(SidecarError::Malformed(_)) => Failure::Invalid,
        _ => Failure::Permanent,
    }
}

fn ready(answer: WireAnswer) -> std::result::Result<Vec<u8>, NativeError> {
    match answer {
        WireAnswer::Ready { payload } if payload.capacity() <= harmos_sidecar::Frame::LIMIT => {
            Ok(payload)
        }
        WireAnswer::Ready { .. } => Err(SidecarError::Malformed(
            "native operation response exceeds the frame limit".to_owned(),
        )
        .into()),
        WireAnswer::Failed { failure, message } => Err(NativeError::Remote { failure, message }),
        WireAnswer::Stream { .. } => Err(SidecarError::Malformed(
            "unexpected stream response to unary phase".to_owned(),
        )
        .into()),
    }
}

fn acknowledged(answer: WireAnswer) -> std::result::Result<(), NativeError> {
    let payload = ready(answer)?;
    if !payload.is_empty() {
        let error = SidecarError::Malformed(
            "native control acknowledgement contains an unexpected payload".to_owned(),
        );
        return Err(error.into());
    }
    Ok(())
}

/* === Tests === */
