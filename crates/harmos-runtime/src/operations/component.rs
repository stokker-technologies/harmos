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
use std::sync::Arc;
// Third Party Imports
use tokio::sync::mpsc;
// Local Imports
use super::Context;
use super::children::{CHILD_REQUESTS, ChildRequest, Children};
use crate::guest::Error as GuestError;
use crate::guest::host::session::Endpoint;
use crate::operations::{FailureKind, OperationDefinition, WireAnswer, WireCommand};
use crate::{Error, Identifier, operations::Lane};
use harmos_operations::execution::operations::Carrier;

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
    fingerprint: [u8; 32],
}

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
}

#[derive(Debug, thiserror::Error)]
pub enum ComponentError {
    #[error(transparent)]
    Dependency(#[from] harmos_operations::execution::ResourceError),

    #[error("the selected guest generation is unavailable")]
    Unavailable,

    #[error("guest attempt context entropy is unavailable")]
    Entropy(#[from] getrandom::Error),

    #[error("operation `{operation}` differs from the selected guest catalog")]
    Catalog { operation: &'static str },

    #[error(transparent)]
    Declaration(#[from] harmos_operations::operations::Error),

    #[error("guest operation transport failed")]
    Transport(#[from] GuestError),

    #[error("guest operation response codec failed")]
    Payload(#[from] crate::MessageError),

    // The guest refused the request while preparing it.
    #[error(transparent)]
    Preparation(harmos_operations::Failure),

    #[error("guest operation request is not granted")]
    Unauthorized(#[source] crate::Error),

    // The guest handler failed; its failure crosses unchanged.
    #[error(transparent)]
    Remote(harmos_operations::Failure),
}

/* === Traits === */

/* === Implementations === */

impl ComponentError {
    pub(crate) fn classification(&self) -> FailureKind {
        classify(self)
    }

    pub(crate) fn failure(&self) -> Option<&harmos_operations::Failure> {
        match self {
            Self::Preparation(failure) | Self::Remote(failure) => Some(failure),
            _ => None,
        }
    }

    // The host adds the one operation frame where the call crossed from the guest.
    fn in_operation(self, operation: &str) -> Self {
        match self {
            Self::Preparation(failure) => Self::Preparation(failure.in_operation(operation)),
            Self::Remote(failure) => Self::Remote(failure.in_operation(operation)),
            error => error,
        }
    }
}

impl Component {
    pub(crate) fn new(endpoint: Arc<Endpoint>) -> Self {
        Self { endpoint }
    }
    pub(crate) fn register(&self, bindings: &mut super::Bindings<'_>, lane: Lane) -> crate::Result<()> {
        let scope = self.scope().ok_or(ComponentError::Unavailable).map_err(|source| Error::OperationFailed {
            operation: "guest registration".into(),
            source: Arc::new(source),
        })?;
        for contract in scope.wire_contracts() {
            let binding = Arc::new(SdkBinding {
                component: Arc::new(Self { endpoint: Arc::clone(&self.endpoint) }),
                definition: contract.operation().clone(),
            });
            let mut dependencies = harmos_operations::execution::__private::ResourceSet::new();
            dependencies.insert(binding)?;
            let mut entry = bindings.operations.bind_native::<super::sdk::Encoded, SdkOperation>(
                scope.owner(),
                lane.name(),
                dependencies,
                contract.operation().clone(),
                Some(sdk_dispatch),
                Some(scope.clone()),
            )?;
            Arc::get_mut(&mut entry).expect("unpublished guest entry").contract = Some(contract.clone());
            bindings.entries.push(entry);
        }
        Ok(())
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
            let scope = self.scope().ok_or(ComponentError::Unavailable)?;
            let operation = format!("{}.{}", scope.owner(), state.operation);
            let fingerprint = state.fingerprint;
            let generation = context.generation.value();
            let command = WireCommand::Prepare {
                invocation,
                operation,
                fingerprint,
                generation,
                payload,
            };
            let answer = self.endpoint.phase(command, None).await?;
            let answer = match answer {
                WireAnswer::Failed { failure } => {
                    return Err(ComponentError::Preparation(failure));
                }
                answer => answer,
            };
            // The guest decoded the request; refuse it before any attempt runs.
            acknowledged(answer)?;
            context.admit_grant(None).map_err(ComponentError::Unauthorized)?;
        }
        let answer = self.phase_encoded(state, context, Phase::Initialize).await?;
        match answer {
            WireAnswer::Failed { failure } => Err(ComponentError::Preparation(failure)),
            answer => acknowledged(answer),
        }
    }

    #[tracing::instrument(target = "harmos::carrier", name = "carrier.phase_encoded", level = "debug", skip_all, fields(invocation = tracing::field::Empty, operation = %state.operation, phase = match &phase { Phase::Initialize => "initialize", Phase::Attempt => "attempt" }))]
    async fn phase_encoded<'ctx>(
        &self, state: &ComponentState, context: &Context<'ctx>, phase: Phase,
    ) -> Result<WireAnswer, ComponentError> {
        let invocation = state.invocation.expect("prepared identity is retained");
        tracing::Span::current().record("invocation", invocation);
        let mut token = [0; 32];
        getrandom::fill(&mut token)?;
        let (requests, mut incoming) = mpsc::channel::<ChildRequest>(CHILD_REQUESTS);
        let children = Children {
            context: token,
            requests,
            submitted: Default::default(),
        };
        let command = match phase {
            Phase::Initialize => WireCommand::Initialize { invocation, context: token },
            Phase::Attempt => WireCommand::Attempt {
                invocation,
                context: token,
                values: context.context().portable(),
            },
        };
        let running = self.endpoint.phase(command, Some(children));
        tokio::pin!(running);
        let mut cancellation_sent = false;
        let answer = loop {
            tokio::select! {
                biased;
                answer = &mut running => break answer?,
                _ = context.scope().cancellation().cancelled(), if !cancellation_sent => {
                    cancellation_sent = true;
                    self.endpoint.phase(WireCommand::Cancel { invocation }, None).await?;
                }
                Some(request) = incoming.recv() => {
                    if request.answer.is_closed() { continue; }
                    let child = context.submit_encoded_at(&request.operation, (request.fingerprint, request.generation), request.payload, request.submitted, request.values, request.policy);
                    // A Cancelled Admission Reply Still Cancels Its Admitted Child
                    if let Err(Ok(child)) = request.answer.send(child) { child.request_cancel(); }
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
    pub fn scope(&self) -> Option<&crate::guest::host::plugin::PluginScope> {
        self.endpoint.scope()
    }

    pub fn stage_sdk(&self, operations: &super::Manager, lane: Lane) -> crate::Result<super::PluginSdkRegistration> {
        let scope = self.scope().ok_or(Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "guest endpoint has no original plugin generation",
        })?;
        let pair = scope.lifecycle().ok_or(Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "generation has no frozen SDK pair",
        })?;
        let component = Arc::new(Self { endpoint: Arc::clone(&self.endpoint) });
        let entries = pair
            .iter()
            .map(|descriptor| {
                let handler = Arc::new(SdkBinding {
                    component: Arc::clone(&component),
                    definition: descriptor.contract().operation().clone(),
                });
                let definition = descriptor.contract().operation().clone();
                let mut entry = operations.bind_native::<super::sdk::Encoded, SdkOperation>(
                    scope.owner(),
                    lane.name(),
                    {
                        let mut dependencies = harmos_operations::execution::__private::ResourceSet::new();
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
        let entries = entries.try_into().map_err(|_| Error::InvalidInput {
            field: "SDK lifecycle",
            reason: "expected exactly two entries",
        })?;
        super::PluginSdkRegistration::new(operations.clone(), scope, entries, crate::guest::host::plugin::PluginKind::Guest)
    }
}

impl Carrier for SdkOperation {
    type Declaration = super::sdk::Encoded;
    type Error = ComponentError;
    type StreamEndpoints = ();

    #[tracing::instrument(target = "harmos::operations", name = "operations.initialize", level = "debug", skip_all)]
    async fn bind(context: harmos_operations::execution::ExecutionContext) -> Result<Self, ComponentError> {
        let binding = context.resource::<SdkBinding>()?;
        Ok(Self { binding, state: None })
    }

    #[tracing::instrument(target = "harmos::operations", name = "operations.run", level = "debug", skip_all)]
    async fn attempt(
        &mut self, context: harmos_operations::execution::ExecutionContext, payload: &mut super::sdk::Encoded,
        _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<crate::operations::Response<Vec<u8>>, ComponentError> {
        let host = context.resource::<super::HostContext>()?;
        let context = host.borrow();
        let result: ::core::result::Result<Vec<u8>, Self::Error> = crate::clock::waiting(async {
            if self.state.is_none() {
                let binding = &self.binding;

                if !binding.component.endpoint.accepts() {
                    return Err(ComponentError::Unavailable);
                }
                let matches = binding.component.endpoint.definitions().contains(&binding.definition)
                    || binding
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
                    fingerprint: binding.definition.fingerprint.0,
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
        })
        .await;
        let operation = self.binding.definition.id.as_str();
        result.map(::core::convert::Into::into).map_err(|error| error.in_operation(operation))
    }

    async fn cleanup(&mut self) -> Result<(), ComponentError> {
        let Some(state) = self.state.as_mut() else {
            return Ok(());
        };

        let operation = self.binding.definition.id.as_str();
        crate::clock::waiting(self.binding.component.finalize_encoded(state)).await.map_err(|error| error.in_operation(operation))
    }

    fn classify(error: &ComponentError) -> FailureKind {
        classify(error)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn sdk_dispatch(
    operations: &super::Manager, mut submission: super::manager::Submission, source: super::caller::Source,
    parent: Option<super::wire::Parent<'_>>, input: super::wire::EncodedInput,
) -> crate::Result<super::EncodedInvocation> {
    submission.preparation = match input.preparation {
        Some(preparation) => Some(preparation),
        None => Some(operations.claim_preparation(std::num::NonZeroUsize::new(input.payload.capacity().max(1)).unwrap())?),
    };
    let (scope, ancestors) = parent.map_or((None, &[][..]), |parent| (Some(parent.scope), parent.ancestors));
    let mut request = super::binding::Request::from(super::sdk::Encoded(input.payload));
    request.retry = input.retry.or(input.policy.map(|policy| policy.retries));
    request.policy = input.policy;
    let invocation = operations.admit_entry::<super::sdk::Encoded, ()>((request, ()), scope, ancestors, &source, submission)?;
    let id = invocation.id();
    let generation = invocation.generation();
    let control = Arc::clone(&invocation.control);
    let invocation = invocation.unguarded();
    let outcome = Box::pin(async move {
        let payload = invocation.await?;
        payload.try_unwrap().map_err(|_| Error::InvalidInput {
            field: "SDK response",
            reason: "SDK response unexpectedly shared",
        })
    });
    Ok(super::EncodedInvocation { id, generation, outcome, control })
}

fn classify(error: &ComponentError) -> FailureKind {
    match error {
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
