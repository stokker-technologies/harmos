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
use crate::operations::{
    Declarations, Definition, Failure, OperationDeclaration, WireAnswer, WireCommand, WireOperation,
};
use crate::plugin::guest::Error as GuestError;
use crate::plugin::guest::host::session::Endpoint;
use crate::schema::Described;
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
    definition: Definition,
}

struct Project<'a, 'b> {
    bindings: &'a mut super::Bindings<'b>,
    lane: &'static str,
    resources: Arc<Component>,
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
    Stream(#[from] super::RemoteStreamError),
    #[error("the selected guest generation is unavailable")]
    Unavailable,
    #[error("guest attempt context entropy is unavailable: {0}")]
    Entropy(#[from] getrandom::Error),
    #[error("operation `{operation}` differs from the selected guest catalog")]
    Catalog { operation: &'static str },
    #[error(transparent)]
    Declaration(#[from] harmos_contract::Error),
    #[error("guest operation transport failed: {0}")]
    Transport(#[from] GuestError),
    #[error("guest operation response codec failed: {0}")]
    Payload(#[from] crate::MessageError),
    #[error("guest operation preparation failed: {message}")]
    Preparation { failure: Failure, message: String },
    #[error("guest operation failed: {message}")]
    Remote { failure: Failure, message: String },
}

/* === Traits === */

/* === Implementations === */
impl ComponentError {
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
            .register_owned_wire_at::<ComponentOperation<O, ()>>(self.lane, self.resources)
    }

    fn upload<O>(self) -> Self::Entry
    where
        O: WireOperation + crate::operations::InputDeclaration,
        O::Request: Message + Described,
        O::Response: Message + Described,
        O::Item: Message + crate::MemorySize,
    {
        self.bindings
            .register_owned_wire_at::<ComponentOperation<O, super::StreamInput<O::Item>>>(
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
            .register_owned_wire_at::<ComponentOperation<O, super::StreamOutput<O::Item>>>(
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
        self.bindings.register_owned_wire_at::<ComponentOperation<
            O,
            (
                super::StreamInput<<O as crate::operations::InputDeclaration>::Item>,
                super::StreamOutput<<O as crate::operations::OutputDeclaration>::Item>,
            ),
        >>(self.lane, self.resources)
    }
}

impl super::StreamPeer for Component {
    type Error = ComponentError;
    async fn stream_phase(
        &self,
        command: WireCommand,
    ) -> std::result::Result<WireAnswer, Self::Error> {
        self.endpoint
            .phase(command, None)
            .await
            .map_err(ComponentError::from)
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
    O::Request: Message,
    O::Response: Message,
{
    type Declaration = O;
    type Resources = Component;
    type Error = ComponentError;
    type Endpoints = E;
    type Context<'a> = Context<'a>;

    fn new(component: Arc<Component>) -> Result<Self, ComponentError> {
        Ok(Self {
            component,
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
    ) -> Result<O::Response, ComponentError> {
        if self.state.is_none() {
            let component = &self.component;

            if !component.endpoint.accepts() {
                return Err(ComponentError::Unavailable);
            }
            let definition = Definition::of::<O>()?;
            if !component.endpoint.definitions().contains(&definition) {
                return Err(ComponentError::Catalog { operation: O::ID });
            }
            let state = ComponentState {
                initialized: false,
                payload: Some(request.encode_to_vec()),
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
            self.component
                .phase_encoded(state, &context, Phase::Attempt)
                .await?
        } else {
            self.epoch = self
                .epoch
                .checked_add(1)
                .ok_or(super::RemoteStreamError::Protocol(
                    "stream attempt counter exhausted",
                ))?;
            let running = self
                .component
                .phase_encoded(state, &context, Phase::Stream(self.epoch));
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

    async fn finalize(&mut self) -> Result<(), ComponentError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        self.component.finalize_encoded(state).await
    }

    fn transferred(&self) -> bool {
        self.transferred.load(Ordering::Acquire)
    }

    fn classify(error: &ComponentError) -> Failure {
        classify(error)
    }
}

impl Visitor for ComponentBindings {
    type Entry = super::Binding<Component>;
}

impl<O> Member<O> for ComponentBindings
where
    O: WireOperation,
    O::Request: Message + Described,
    O::Response: Message + Described,
{
    const ENTRY: super::Binding<Component> = super::Binding {
        register: register::<O>,
    };
}

impl super::Bindings<'_> {
    pub fn register_component_catalog<C>(
        &mut self,
        lane: impl Lane,
        component: Arc<Component>,
    ) -> crate::Result<()>
    where
        C: Catalog<Declarations> + Catalog<ComponentBindings>,
    {
        let scope = self.scope.ok_or(Error::InvalidInput {
            field: "component operation scope",
            reason: "component inventory requires stage_plugin",
        })?;
        if !component
            .scope()
            .is_some_and(|selected| selected.same_activation(scope))
        {
            return Err(Error::InvalidInput {
                field: "component operation scope",
                reason: "component endpoint belongs to a different plugin generation",
            });
        }
        self.project::<C, ComponentBindings, Component>(lane, component)
    }
}

impl Component {
    async fn initialize_encoded<'ctx>(
        &self,
        state: &mut ComponentState,
        context: &Context<'ctx>,
    ) -> Result<(), ComponentError> {
        if state.invocation.is_none() {
            let invocation = context.id().value();
            // Record Ownership Before Sending: A Lost Ack May Still Own Remote State
            state.invocation = Some(invocation);
            let payload = state.payload.take().expect("one inert request is owned");
            let command = WireCommand::Prepare {
                invocation,
                operation: state.operation.to_string(),
                payload,
            };
            let answer = self.endpoint.phase(command, None).await?;
            let answer = match answer {
                WireAnswer::Failed { failure, message } => {
                    return Err(ComponentError::Preparation { failure, message });
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
                Err(ComponentError::Preparation { failure, message })
            }
            answer => acknowledged(answer),
        }
    }

    async fn phase_encoded<'ctx>(
        &self,
        state: &ComponentState,
        context: &Context<'ctx>,
        phase: Phase,
    ) -> Result<WireAnswer, ComponentError> {
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

    async fn finalize_encoded(&self, state: &mut ComponentState) -> Result<(), ComponentError> {
        let Some(invocation) = state.invocation else {
            return Ok(());
        };
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

    pub fn stage_sdk(
        &self,
        operations: &super::OperationManager,
        lane: impl Lane,
    ) -> crate::Result<super::PluginSdkRegistration> {
        let scope = self.scope().ok_or(Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "guest endpoint has no original plugin generation",
        })?;
        let pair = scope.lifecycle().ok_or(Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "generation has no frozen SDK pair",
        })?;
        let component = Arc::new(Self {
            endpoint: Arc::clone(&self.endpoint),
        });
        let entries = pair
            .iter()
            .map(|descriptor| {
                let handler = Arc::new(SdkBinding {
                    component: Arc::clone(&component),
                    definition: descriptor.contract().operation().clone(),
                });
                let mut definition = descriptor.contract().operation().clone();
                let suffix = match descriptor.action() {
                    harmos_contract::operations::lifecycle::Action::Initialize => "initialize",
                    harmos_contract::operations::lifecycle::Action::Shutdown => "shutdown",
                };
                definition.id = crate::Identifier::new(format!("{}.sdk.{suffix}", scope.owner()))
                    .map_err(|source| Error::OperationDeclaration {
                    source: harmos_contract::Error::from(source),
                })?;
                let mut entry = operations.binding_owned_definition::<SdkOperation>(
                    scope.owner(),
                    lane.name(),
                    handler,
                    definition,
                    Some(sdk_dispatch),
                    Some(scope.clone()),
                )?;
                Arc::get_mut(&mut entry)
                    .expect("new SDK entry has one owner")
                    .contract = Some(descriptor.contract().clone());
                Ok(entry)
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let entries = entries.try_into().map_err(|_| Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "expected exactly two entries",
        })?;
        super::PluginSdkRegistration::new(
            operations.clone(),
            scope,
            entries,
            crate::plugin::PluginKind::Guest,
        )
    }
}

impl Operation for SdkOperation {
    type Declaration = super::sdk::Encoded;
    type Resources = SdkBinding;
    type Error = ComponentError;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    fn new(binding: Arc<SdkBinding>) -> Result<Self, ComponentError> {
        Ok(Self {
            binding,
            state: None,
        })
    }

    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        payload: &mut Vec<u8>,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<Vec<u8>, ComponentError> {
        if self.state.is_none() {
            let binding = &self.binding;

            if !binding.component.endpoint.accepts() {
                return Err(ComponentError::Unavailable);
            }
            let matches = binding
                .component
                .scope()
                .and_then(|scope| scope.lifecycle())
                .is_some_and(|pair| {
                    pair.iter()
                        .any(|descriptor| descriptor.contract().operation() == &binding.definition)
                });
            if !matches {
                return Err(ComponentError::Catalog {
                    operation: "SDK lifecycle",
                });
            }
            let state = ComponentState {
                initialized: false,
                payload: Some(std::mem::take(payload)),
                invocation: None,
                operation: binding.definition.id.clone(),
            };

            self.state = Some(state);
        }
        let state = self.state.as_mut().expect("original remote state");
        if !state.initialized {
            self.binding
                .component
                .initialize_encoded(state, &context)
                .await?;
            state.initialized = true;
        }

        let answer = self
            .binding
            .component
            .phase_encoded(state, &context, Phase::Attempt)
            .await?;
        ready(answer)
    }

    async fn finalize(&mut self) -> Result<(), ComponentError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        self.binding.component.finalize_encoded(state).await
    }

    fn classify(error: &ComponentError) -> Failure {
        classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn register<O>(
    bindings: &mut super::Bindings<'_>,
    lane: &'static str,
    component: Arc<Component>,
) -> crate::Result<()>
where
    O: WireOperation,
    O::Request: Message + Described,
    O::Response: Message + Described,
{
    if let Some(projected) = O::visit_wire(Project {
        bindings,
        lane,
        resources: Arc::clone(&component),
    }) {
        projected
    } else {
        bindings.register_owned_wire_at::<ComponentOperation<O>>(lane, component)
    }
}

fn sdk_dispatch(
    operations: &super::OperationManager,
    mut submission: super::manager::Submission,
    source: super::caller::Source,
    parent: Option<super::wire::Parent<'_>>,
    input: super::wire::EncodedInput,
) -> crate::Result<super::EncodedInvocation> {
    submission.preparation = input.preparation;
    let (scope, ancestors) = parent.map_or((None, &[][..]), |parent| {
        (Some(parent.scope), parent.ancestors)
    });
    let invocation = operations.admit_entry::<super::sdk::Encoded, ()>(
        (input.payload.into(), ()),
        scope,
        ancestors,
        &source,
        submission,
    )?;
    let id = invocation.id();
    let generation = invocation.generation();
    let control = Arc::clone(&invocation.control);
    let outcome = Box::pin(async move {
        let payload = invocation.await?;
        Arc::try_unwrap(payload).map_err(|_| Error::InvalidInput {
            field: "SDK response",
            reason: "uncached SDK response unexpectedly shared",
        })
    });
    Ok(super::EncodedInvocation {
        id,
        generation,
        outcome,
        control,
    })
}

fn classify(error: &ComponentError) -> Failure {
    match error {
        ComponentError::Stream(source) => source.classification(),
        ComponentError::Remote { failure, .. } => *failure,
        ComponentError::Preparation { failure, .. } => match failure {
            Failure::Invalid | Failure::Unauthorized | Failure::Cancelled => *failure,
            _ => Failure::Permanent,
        },
        ComponentError::Transport(
            GuestError::Unavailable { .. } | GuestError::Execution { .. },
        ) => Failure::Ambiguous,
        ComponentError::Payload(_)
        | ComponentError::Declaration(_)
        | ComponentError::Catalog { .. }
        | ComponentError::Transport(GuestError::Malformed { .. }) => Failure::Invalid,
        _ => Failure::Permanent,
    }
}

fn ready(answer: WireAnswer) -> Result<Vec<u8>, ComponentError> {
    match answer {
        WireAnswer::Ready { payload } => Ok(payload),
        WireAnswer::Failed { failure, message } => Err(ComponentError::Remote { failure, message }),
        WireAnswer::Stream { .. } => Err(GuestError::Malformed {
            method: "operate".to_owned(),
            message: "unexpected stream response to unary phase".to_owned(),
        }
        .into()),
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
