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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
// Third Party Imports
use tokio::sync::mpsc;
// Local Imports
use super::children::{CHILD_REQUESTS, ChildRequest, Children};
use super::{Context, Operation};
use crate::catalog::{Catalog, Member, Visitor};
use crate::guest::Error as GuestError;
use crate::guest::host::session::Endpoint;
use crate::operations::{Declarations, FailureKind, OperationDeclaration, OperationDefinition, WireAnswer, WireCommand};
use crate::{Error, Identifier, Message, operations::Lane};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Component {
    endpoint: Arc<Endpoint>,
}

struct ComponentState {
    initialized: bool,
    payload: Option<Vec<u8>>,
    invocation: Option<u64>,
    operation: Identifier,
}

pub struct ComponentOperation<O: OperationDeclaration, E = ()> {
    component: Arc<Component>,
    state: Option<ComponentState>,
    epoch: u64,
    transferred: AtomicBool,
    _operation: PhantomData<fn() -> (O, E)>,
}

pub struct ComponentBindings;

struct SdkOperation {
    binding: Arc<SdkBinding>,
    state: Option<ComponentState>,
}

struct SdkBinding {
    component: Arc<Component>,
    definition: OperationDefinition,
}

/* === Enums === */

enum Phase {
    Initialize,
    Attempt,
    Stream(u64),
}

#[derive(Debug, thiserror::Error)]
pub enum ComponentError {
    #[error(transparent)]
    Dependency(#[from] harmos_sdk::DependencyError),

    #[error(transparent)]
    Stream(#[from] super::RemoteStreamError),

    #[error("the selected guest generation is unavailable")]
    Unavailable,

    #[error("guest attempt context entropy is unavailable")]
    Entropy(#[from] getrandom::Error),

    #[error("operation `{operation}` differs from the selected guest catalog")]
    Catalog { operation: &'static str },

    #[error(transparent)]
    Declaration(#[from] harmos_domain::Error),

    #[error("guest operation transport failed")]
    Transport(#[from] GuestError),

    #[error("guest operation response codec failed")]
    Payload(#[from] crate::MessageError),

    // The guest refused the request while preparing it.
    #[error(transparent)]
    Preparation(harmos_domain::Failure),

    #[error("guest operation request is not granted")]
    Unauthorized(#[source] crate::Error),

    // The guest handler failed; its failure crosses unchanged.
    #[error(transparent)]
    Remote(harmos_domain::Failure),
}

/* === Traits === */

/* === Implementations === */

impl ComponentError {
    pub(crate) fn classification(&self) -> FailureKind {
        classify(self)
    }

    pub(crate) fn failure(&self) -> Option<&harmos_domain::Failure> {
        match self {
            Self::Preparation(failure) | Self::Remote(failure) => Some(failure),
            Self::Stream(source) => source.failure(),
            _ => None,
        }
    }

    // The host adds the one operation frame where the call crossed from the guest.
    fn in_operation(self, operation: &str) -> Self {
        match self {
            Self::Preparation(failure) => Self::Preparation(failure.in_operation(operation)),
            Self::Remote(failure) => Self::Remote(failure.in_operation(operation)),
            Self::Stream(source) => Self::Stream(source.in_operation(operation)),
            error => error,
        }
    }
}

impl super::StreamPeer for Component {
    type Error = ComponentError;
    async fn stream_phase(&self, command: WireCommand) -> std::result::Result<WireAnswer, Self::Error> {
        self.endpoint.phase(command, None).await.map_err(ComponentError::from)
    }
}

impl Component {
    pub(crate) fn new(endpoint: Arc<Endpoint>) -> Self {
        Self { endpoint }
    }
}

impl<O, E> Operation for ComponentOperation<O, E>
where
    O: OperationDeclaration,
    E: super::StreamEndpoints,
{
    type Request = <O as OperationDeclaration>::Request;
    type Response = <O as OperationDeclaration>::Response;
    type Error = ComponentError;
    type Endpoints = E;

    #[tracing::instrument(target = "harmos::operations", name = "operations.initialize", level = "debug", skip_all)]
    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self, ComponentError> {
        let component = context.dependency::<Component>()?;
        Ok(Self { component, state: None, epoch: 0, transferred: AtomicBool::new(false), _operation: PhantomData })
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.run", level = "debug", skip_all)]
    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut O::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<O::Response>, ComponentError> {
        let host = context.dependency::<super::HostContext>()?;
        let context = host.borrow();
        let result: ::core::result::Result<O::Response, Self::Error> = async move {
            if self.state.is_none() {
                let component = &self.component;

                if !component.endpoint.accepts() {
                    return Err(ComponentError::Unavailable);
                }
                let definition = OperationDefinition::of::<O>()?;
                if !component.endpoint.definitions().contains(&definition) {
                    return Err(ComponentError::Catalog { operation: O::ID });
                }
                let state = ComponentState {
                    initialized: false,
                    payload: Some(crate::Message::encode(request)),
                    invocation: None,
                    operation: definition.id,
                };

                self.state = Some(state);
            }
            let state = self.state.as_mut().expect("original remote state");
            if !state.initialized {
                self.component.initialize_encoded(state, &context).await?;
                state.initialized = true;
            }

            let answer = if E::item_types() == (None, None) {
                self.component.phase_encoded(state, &context, Phase::Attempt).await?
            } else {
                self.epoch = self.epoch.checked_add(1).ok_or(super::RemoteStreamError::Protocol("stream attempt counter exhausted"))?;
                let running = self.component.phase_encoded(state, &context, Phase::Stream(self.epoch));
                super::remote::attempt(
                    &*self.component,
                    endpoints,
                    state.invocation.expect("original prepared invocation"),
                    self.epoch,
                    &self.transferred,
                    running,
                )
                .await?
            };
            let payload = ready(answer)?;
            O::Response::decode(payload.as_slice()).map_err(ComponentError::Payload)
        }
        .await;
        result.map(::core::convert::Into::into).map_err(|error| error.in_operation(O::ID))
    }

    async fn cleanup(&mut self) -> Result<(), ComponentError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        self.component.finalize_encoded(state).await.map_err(|error| error.in_operation(O::ID))
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }

    fn classify(error: &ComponentError) -> FailureKind {
        classify(error)
    }
}

impl Visitor for ComponentBindings {
    type Entry = super::Binding<Component>;
}

impl<O> Member<O> for ComponentBindings
where
    O: crate::operations::OperationDeclaration,
{
    const ENTRY: super::Binding<Component> = super::Binding { register: register::<O> };
}

impl super::Bindings<'_> {
    pub fn register_component_catalog<C>(&mut self, lane: impl Lane, component: Arc<Component>) -> crate::Result<()>
    where
        C: Catalog<Declarations> + Catalog<ComponentBindings>,
    {
        let scope = self
            .scope
            .ok_or(Error::InvalidInput { field: "component operation scope", reason: "component inventory requires stage_plugin" })?;
        if !component.scope().is_some_and(|selected| selected.same_activation(scope)) {
            return Err(Error::InvalidInput {
                field: "component operation scope",
                reason: "component endpoint belongs to a different plugin generation",
            });
        }
        self.project::<C, ComponentBindings, Component>(lane, component)
    }
}

impl Component {
    #[tracing::instrument(target = "harmos::carrier", name = "carrier.initialize_encoded", level = "debug", skip_all, fields(invocation = tracing::field::Empty, operation = %state.operation))]
    async fn initialize_encoded<'ctx>(&self, state: &mut ComponentState, context: &Context<'ctx>) -> Result<(), ComponentError> {
        if state.invocation.is_none() {
            let invocation = context.id().value();
            // Record Ownership Before Sending: A Lost Ack May Still Own Remote State
            state.invocation = Some(invocation);
            tracing::Span::current().record("invocation", invocation);
            let payload = state.payload.take().expect("one inert request is owned");
            let command = WireCommand::Prepare { invocation, operation: state.operation.to_string(), payload };
            let answer = self.endpoint.phase(command, None).await?;
            let answer = match answer {
                WireAnswer::Failed { failure } => {
                    return Err(ComponentError::Preparation(failure));
                }
                answer => answer,
            };
            // The guest decoded the request; refuse it before any attempt runs.
            let payload = ready(answer)?;
            let grant = harmos_sdk::operations::decode_prepared_grant(&payload).map_err(|error| GuestError::Malformed {
                method: "operate".to_owned(),
                message: format!("prepared grant is malformed: {error}"),
            })?;
            context.admit_grant(grant.as_ref()).map_err(ComponentError::Unauthorized)?;
        }
        let answer = self.phase_encoded(state, context, Phase::Initialize).await?;
        match answer {
            WireAnswer::Failed { failure } => Err(ComponentError::Preparation(failure)),
            answer => acknowledged(answer),
        }
    }

    #[tracing::instrument(target = "harmos::carrier", name = "carrier.phase_encoded", level = "debug", skip_all, fields(invocation = tracing::field::Empty, operation = %state.operation, phase = match &phase { Phase::Initialize => "initialize", Phase::Attempt => "attempt", Phase::Stream(_) => "stream_attempt" }))]
    async fn phase_encoded<'ctx>(
        &self, state: &ComponentState, context: &Context<'ctx>, phase: Phase,
    ) -> Result<WireAnswer, ComponentError> {
        let invocation = state.invocation.expect("prepared identity is retained");
        tracing::Span::current().record("invocation", invocation);
        let mut token = [0; 32];
        getrandom::fill(&mut token)?;
        let (requests, mut incoming) = mpsc::channel::<ChildRequest>(CHILD_REQUESTS);
        let children = Children { context: token, requests, submitted: Default::default() };
        let command = match phase {
            Phase::Initialize => WireCommand::Initialize { invocation, context: token },
            Phase::Stream(epoch) => WireCommand::StreamAttempt { invocation, epoch, context: token, values: context.context().portable() },
            Phase::Attempt => WireCommand::Attempt { invocation, context: token, values: context.context().portable() },
        };
        let running = self.endpoint.phase(command, Some(children));
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

    #[tracing::instrument(target = "harmos::carrier", name = "carrier.finalize_encoded", level = "debug", skip_all, fields(invocation = tracing::field::Empty, operation = %state.operation))]
    async fn finalize_encoded(&self, state: &mut ComponentState) -> Result<(), ComponentError> {
        let Some(invocation) = state.invocation else {
            return Ok(());
        };
        tracing::Span::current().record("invocation", invocation);
        if !self.endpoint.accepts() {
            self.endpoint.settle().await?;
            return Ok(());
        }
        let command = WireCommand::Finalize { invocation };
        match self.endpoint.phase(command, None).await {
            Ok(answer) => {
                acknowledged(answer)?;
                Ok(())
            }
            Err(GuestError::Unavailable { .. }) => {
                self.endpoint.settle().await?;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }
}

impl Component {
    pub fn scope(&self) -> Option<&crate::plugin::PluginScope> {
        self.endpoint.scope()
    }

    pub fn stage_sdk(&self, operations: &super::Manager, lane: impl Lane) -> crate::Result<super::PluginSdkRegistration> {
        let scope = self
            .scope()
            .ok_or(Error::InvalidInput { field: "SDK lifecycle", reason: "guest endpoint has no original plugin generation" })?;
        let pair = scope.lifecycle().ok_or(Error::InvalidInput { field: "SDK lifecycle", reason: "generation has no frozen SDK pair" })?;
        let component = Arc::new(Self { endpoint: Arc::clone(&self.endpoint) });
        let entries = pair
            .iter()
            .map(|descriptor| {
                let handler =
                    Arc::new(SdkBinding { component: Arc::clone(&component), definition: descriptor.contract().operation().clone() });
                let mut definition = descriptor.contract().operation().clone();
                let suffix = match descriptor.action() {
                    harmos_domain::operations::lifecycle::Action::Initialize => "initialize",
                    harmos_domain::operations::lifecycle::Action::Shutdown => "shutdown",
                };
                definition.id = crate::Identifier::new(format!("{}.sdk.{suffix}", scope.owner()))
                    .map_err(|source| Error::OperationDeclaration { source: harmos_domain::Error::from(source) })?;
                let mut entry = operations.bind_native::<super::sdk::Encoded, SdkOperation>(
                    scope.owner(),
                    lane.name(),
                    {
                        let mut dependencies = harmos_sdk::Dependencies::new();
                        dependencies.insert(handler)?;
                        dependencies
                    },
                    definition,
                    Some(sdk_dispatch),
                    Some(scope.clone()),
                )?;
                Arc::get_mut(&mut entry).expect("new SDK entry has one owner").contract = Some(descriptor.contract().clone());
                Ok(entry)
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let entries =
            entries.try_into().map_err(|_| Error::InvalidInput { field: "SDK lifecycle", reason: "expected exactly two entries" })?;
        super::PluginSdkRegistration::new(operations.clone(), scope, entries, crate::plugin::PluginKind::Guest)
    }
}

impl Operation for SdkOperation {
    type Request = <super::sdk::Encoded as OperationDeclaration>::Request;
    type Response = <super::sdk::Encoded as OperationDeclaration>::Response;
    type Error = ComponentError;
    type Endpoints = ();

    #[tracing::instrument(target = "harmos::operations", name = "operations.initialize", level = "debug", skip_all)]
    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self, ComponentError> {
        let binding = context.dependency::<SdkBinding>()?;
        Ok(Self { binding, state: None })
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.run", level = "debug", skip_all)]
    async fn run(
        &mut self, context: harmos_sdk::OperationContext, payload: &mut super::sdk::Encoded, _endpoints: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<Vec<u8>>, ComponentError> {
        let host = context.dependency::<super::HostContext>()?;
        let context = host.borrow();
        let result: ::core::result::Result<Vec<u8>, Self::Error> = async {
            if self.state.is_none() {
                let binding = &self.binding;

                if !binding.component.endpoint.accepts() {
                    return Err(ComponentError::Unavailable);
                }
                let matches = binding
                    .component
                    .scope()
                    .and_then(|scope| scope.lifecycle())
                    .is_some_and(|pair| pair.iter().any(|descriptor| descriptor.contract().operation() == &binding.definition));
                if !matches {
                    return Err(ComponentError::Catalog { operation: "SDK lifecycle" });
                }
                let state = ComponentState {
                    initialized: false,
                    payload: Some(std::mem::take(&mut payload.0)),
                    invocation: None,
                    operation: binding.definition.id.clone(),
                };

                self.state = Some(state);
            }
            let state = self.state.as_mut().expect("original remote state");
            if !state.initialized {
                self.binding.component.initialize_encoded(state, &context).await?;
                state.initialized = true;
            }

            let answer = self.binding.component.phase_encoded(state, &context, Phase::Attempt).await?;
            ready(answer)
        }
        .await;
        let operation = self.binding.definition.id.as_str();
        result.map(::core::convert::Into::into).map_err(|error| error.in_operation(operation))
    }

    async fn cleanup(&mut self) -> Result<(), ComponentError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        let operation = self.binding.definition.id.as_str();
        self.binding.component.finalize_encoded(state).await.map_err(|error| error.in_operation(operation))
    }

    fn classify(error: &ComponentError) -> FailureKind {
        classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn register<O>(bindings: &mut super::Bindings<'_>, lane: &'static str, component: Arc<Component>) -> crate::Result<()>
where
    O: crate::operations::OperationDeclaration,
{
    use crate::operations::Direction;
    type In<O> = super::StreamInput<<<O as crate::operations::OperationDeclaration>::Input as Direction>::Item>;
    type Out<O> = super::StreamOutput<<<O as crate::operations::OperationDeclaration>::Output as Direction>::Item>;
    let mut dependencies = harmos_sdk::Dependencies::new();
    dependencies.insert(component)?;
    match (<O::Input as Direction>::STREAMED, <O::Output as Direction>::STREAMED) {
        (false, false) => bindings.stage_implementation::<O, ComponentOperation<O, ()>>(lane, dependencies),
        (true, false) => bindings.stage_implementation::<O, ComponentOperation<O, In<O>>>(lane, dependencies),
        (false, true) => bindings.stage_implementation::<O, ComponentOperation<O, Out<O>>>(lane, dependencies),
        (true, true) => bindings.stage_implementation::<O, ComponentOperation<O, (In<O>, Out<O>)>>(lane, dependencies),
    }
}

fn sdk_dispatch(
    operations: &super::Manager, mut submission: super::manager::Submission, source: super::caller::Source,
    parent: Option<super::wire::Parent<'_>>, input: super::wire::EncodedInput,
) -> crate::Result<super::EncodedInvocation> {
    submission.preparation = input.preparation;
    let (scope, ancestors) = parent.map_or((None, &[][..]), |parent| (Some(parent.scope), parent.ancestors));
    let invocation = operations.admit_entry::<super::sdk::Encoded, ()>(
        (super::sdk::Encoded(input.payload).into(), ()),
        scope,
        ancestors,
        &source,
        submission,
    )?;
    let id = invocation.id();
    let generation = invocation.generation();
    let control = Arc::clone(&invocation.control);
    let invocation = invocation.unguarded();
    let outcome = Box::pin(async move {
        let payload = invocation.await?;
        payload.try_unwrap().map_err(|_| Error::InvalidInput { field: "SDK response", reason: "uncached SDK response unexpectedly shared" })
    });
    Ok(super::EncodedInvocation { id, generation, outcome, control })
}

fn classify(error: &ComponentError) -> FailureKind {
    match error {
        ComponentError::Stream(source) => source.classification(),
        ComponentError::Remote(failure) => failure.code(),
        ComponentError::Unauthorized(_) => FailureKind::Unauthorized,
        ComponentError::Preparation(failure) => match failure.code() {
            code @ (FailureKind::Invalid | FailureKind::Unauthorized | FailureKind::Cancelled) => code,
            _ => FailureKind::Permanent,
        },
        ComponentError::Transport(GuestError::Unavailable { .. } | GuestError::Execution { .. }) => FailureKind::Ambiguous,
        ComponentError::Payload(_)
        | ComponentError::Declaration(_)
        | ComponentError::Catalog { .. }
        | ComponentError::Transport(GuestError::Malformed { .. }) => FailureKind::Invalid,
        _ => FailureKind::Permanent,
    }
}

fn ready(answer: WireAnswer) -> Result<Vec<u8>, ComponentError> {
    match answer {
        WireAnswer::Ready { payload } => Ok(payload),
        WireAnswer::Failed { failure } => Err(ComponentError::Remote(failure)),
        WireAnswer::Stream { .. } => {
            Err(GuestError::Malformed { method: "operate".to_owned(), message: "unexpected stream response to unary phase".to_owned() }
                .into())
        }
    }
}

fn acknowledged(answer: WireAnswer) -> Result<(), ComponentError> {
    let payload = ready(answer)?;
    if !payload.is_empty() {
        let error = GuestError::Malformed {
            method: "operate".to_owned(),
            message: "guest control acknowledgement contains an unexpected payload".to_owned(),
        };
        return Err(error.into());
    }
    Ok(())
}

/* === Tests === */
