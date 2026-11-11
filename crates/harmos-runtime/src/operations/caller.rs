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
#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
use std::sync::{Arc, Weak};
// Third Party Imports

// Local Imports
#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
use super::manager::Entry;

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
use super::{Manager, Running};

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
use crate::Error;
use crate::Result;

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
use crate::plugin::{PluginCaller, PluginGeneration, PluginScope};

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
use crate::operations::OperationDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
#[derive(Clone)]
pub struct Caller {
    pub(crate) operations: Manager,
    pub(super) source: PluginCaller,
}

#[derive(Clone)]
pub(super) struct Origin {
    pub(super) identity: Identity,

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    pub(super) scope: Option<PluginScope>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    pub(super) lifecycle: Option<LifecycleLoan>,

    // Calls made by a resident control, which may publish while it retires.
    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    pub(super) resident: bool,

    // The grant authority admission recorded for the request this origin made.
    pub(super) grants: Option<std::sync::Arc<super::manager::GrantAuthority>>,
}

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
#[derive(Clone)]
pub(super) struct LifecycleLoan {
    shutdown: Weak<Entry>,
    control: Weak<crate::supervision::instance::Instance>,
}

/* === Enums === */

#[derive(Clone)]
pub(super) enum Source {
    Host,

    #[cfg(feature = "grpc")]
    External {
        source: Box<Source>,
        id: super::OperationInstanceId,
    },

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    Plugin(PluginCaller),

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    Scope(PluginScope),

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    Binding(Weak<Entry>),

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    Callback(Weak<Entry>),

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    Lifecycle {
        binding: Weak<Entry>,
        loan: LifecycleLoan,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Identity {
    Host,

    #[cfg(feature = "grpc")]
    External(super::OperationInstanceId),

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    Plugin {
        owner: Arc<str>,
        generation: PluginGeneration,
    },
}

/* === Traits === */

/* === Implementations === */

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
impl Manager {
    pub fn caller(&self, source: &PluginCaller) -> Result<Caller> {
        Source::Plugin(source.clone()).check()?;
        Ok(Caller {
            operations: self.clone(),
            source: source.clone(),
        })
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
impl Caller {
    pub fn submit_at<O: OperationDeclaration>(&self, id: &str, request: O::Request) -> Result<super::Running<O>> {
        self.operations.admit_at::<O, ()>(id, (request.into(), ()), None, &[], &Source::Plugin(self.source.clone()), None)
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn submit_external_charged<O: OperationDeclaration, P: Send + 'static>(
        &self, request: super::StreamItem<O::Request>, endpoints: P,
    ) -> Result<super::Running<O>> {
        self.operations
            .submit_by::<O, P>((super::binding::Request::charged(request)?, endpoints), &Source::Plugin(self.source.clone()).external()?)
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn with_context(&self, context: super::CallContext) -> Self {
        Self {
            operations: self.operations.with_context(context),
            source: self.source.clone(),
        }
    }

    pub fn generation(&self) -> PluginGeneration {
        self.source.generation()
    }

    pub fn submit_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<super::EncodedInvocation> {
        self.operations.submit_encoded_as(operation, payload, Source::Plugin(self.source.clone()), None)
    }

    pub fn submit_encoded_with(
        &self, operation: &str, payload: Vec<u8>, retry: Option<super::RetryPolicy>,
    ) -> Result<super::EncodedInvocation> {
        self.operations.submit_encoded_retry_as(operation, payload, Source::Plugin(self.source.clone()), retry)
    }

    pub async fn call_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<Vec<u8>> {
        self.submit_encoded(operation, payload)?.await
    }

    pub fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<Running<O>> {
        self.operations.submit_by::<O, ()>((request.into(), ()), &Source::Plugin(self.source.clone()))
    }

    pub fn submit_endpoints<O: OperationDeclaration, P: Send + 'static>(&self, request: O::Request, endpoints: P) -> Result<Running<O>> {
        self.operations.submit_by::<O, P>((request.into(), endpoints), &Source::Plugin(self.source.clone()))
    }

    pub fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self, request: super::StreamItem<O::Request>, endpoints: P,
    ) -> Result<Running<O>> {
        self.operations.submit_by::<O, P>((super::binding::Request::charged(request)?, endpoints), &Source::Plugin(self.source.clone()))
    }

    pub fn operation<O: OperationDeclaration>(&self, request: O::Request) -> harmos_operations::execution::operations::Invocation<Self, O> {
        harmos_operations::execution::operations::Invocation::new(self.clone(), request)
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
impl harmos_operations::execution::operations::OperationHost for Caller {
    type Error = Error;
    type Instance<O: OperationDeclaration> = Running<O>;
    fn context(&self) -> &super::CallContext {
        self.operations.call_context()
    }
    fn with_value<T: super::ContextValue>(&self, value: T) -> std::result::Result<Self, super::ContextError> {
        Ok(Self {
            operations: self.operations.with_context(self.operations.call_context().with_value(value)?),
            source: self.source.clone(),
        })
    }
    async fn invoke<O: OperationDeclaration, E: super::EndpointSet>(
        &self, request: O::Request, retry: Option<super::RetryPolicy>, endpoints: E,
    ) -> Result<Running<O>> {
        super::surface::validate_endpoints::<O, E>()?;
        let mut request = super::binding::Request::from(request);
        request.retry = retry;
        self.operations.submit_by::<O, E>((request, endpoints), &Source::Plugin(self.source.clone()))
    }
    async fn submit<O: OperationDeclaration>(&self, request: O::Request) -> Result<Running<O>> {
        Caller::submit::<O>(self, request)
    }
    async fn submit_owned<O: OperationDeclaration>(&self, request: O::Request, claims: super::RequestClaims) -> Result<Running<O>> {
        self.operations.submit_by::<O, ()>((super::binding::Request::owned(request, claims), ()), &Source::Plugin(self.source.clone()))
    }
    async fn publish_on<S: harmos_domain::StreamSpec>(&self, row: &S::Row) -> Result<()>
    where
        S::Row: harmos_domain::Message,
    {
        harmos_operations::streams::publish_on::<_, S>(self, row).await
    }
}

impl Source {
    #[cfg(feature = "grpc")]
    pub(super) fn external(self) -> Result<Self> {
        Ok(Self::External {
            source: Box::new(self),
            id: super::handler::fresh_instance()?,
        })
    }

    pub(super) fn check(&self) -> Result<Origin> {
        #[cfg(feature = "grpc")]
        if let Self::External { source, id } = self {
            let mut origin = source.check()?;
            origin.identity = Identity::External(*id);
            return Ok(origin);
        }

        #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
        let scope = match self {
            Self::Host => None,
            #[cfg(feature = "grpc")]
            Self::External { .. } => unreachable!("external origin was checked"),
            Self::Scope(scope) => {
                if !scope.is_ready() {
                    return Err(Error::Closing { domain: "plugin scope" });
                }
                Some(scope.clone())
            }
            Self::Plugin(source) => Some(source.operation_scope().map_err(|source| Error::OperationCaller { source })?),
            Self::Lifecycle { binding, loan } => {
                if !loan.is_live() {
                    return Err(Error::Closing { domain: "SDK lifecycle loan" });
                }
                let binding = binding.upgrade().ok_or(Error::Closing { domain: "operation caller" })?;
                binding.available_for_lifecycle()?;
                Some(binding.plugin().cloned().ok_or(Error::Closing { domain: "plugin lifecycle caller" })?)
            }
            Self::Binding(binding) | Self::Callback(binding) => {
                let binding = binding.upgrade().ok_or(Error::Closing { domain: "operation caller" })?;
                binding.available()?;
                binding.plugin().cloned()
            }
        };
        Ok(Origin {
            #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
            identity: scope.as_ref().map_or(Identity::Host, |scope| Identity::Plugin {
                owner: scope.owner_identity(),
                generation: scope.generation(),
            }),

            #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
            scope,
            #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
            lifecycle: match self {
                Self::Lifecycle { loan, .. } => Some(loan.clone()),
                _ => None,
            },
            #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
            resident: match self {
                Self::Binding(binding) | Self::Callback(binding) => binding.upgrade().is_some_and(|binding| binding.retires_late()),
                _ => false,
            },
            grants: None,
        })
    }

    pub(super) fn for_attempt(
        &self, entry: &std::sync::Arc<super::manager::Entry>, _scope: crate::operations::Scope<'_>, _origin: &Origin,
    ) -> Self {
        #[cfg(feature = "grpc")]
        if let Self::External { source, id } = self {
            return Self::External {
                source: Box::new(source.for_attempt(entry, _scope, _origin)),
                id: *id,
            };
        }
        // A Host Provider Serving a Plugin Requirement Acts for That Caller
        #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
        if entry.plugin().is_none() && _origin.scope.is_some() {
            return self.clone();
        }
        #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
        if entry.plugin().is_some() {
            let control = _scope.original_control();
            let loan = if control.upgrade().is_some_and(|control| entry.admitted_shutdown(&control)) {
                Some(LifecycleLoan { shutdown: Arc::downgrade(entry), control })
            } else {
                _origin.lifecycle.clone()
            };
            if let Some(loan) = loan {
                return Self::Lifecycle { binding: Arc::downgrade(entry), loan };
            }
        }
        Self::for_binding(entry)
    }

    pub(super) fn for_binding(_entry: &std::sync::Arc<super::manager::Entry>) -> Self {
        #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
        if _entry.plugin().is_some() {
            return Self::Binding(Arc::downgrade(_entry));
        }
        Self::Host
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
    pub(super) fn for_callback(entry: &Arc<Entry>) -> Self {
        if entry.plugin().is_some() { Self::Callback(Arc::downgrade(entry)) } else { Self::Host }
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest"))]
impl LifecycleLoan {
    fn is_live(&self) -> bool {
        let Some(entry) = self.shutdown.upgrade() else {
            return false;
        };
        let Some(control) = self.control.upgrade() else {
            return false;
        };
        entry.admitted_shutdown(&control)
    }
}

impl Origin {
    pub(super) fn has_lifecycle_loan(&self) -> bool {
        #[cfg(any(feature = "plugin", feature = "plugin-guest"))]
        {
            self.lifecycle.as_ref().is_some_and(LifecycleLoan::is_live)
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
