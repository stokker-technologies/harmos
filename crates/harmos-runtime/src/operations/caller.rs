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
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use std::sync::{Arc, Weak};
// Third Party Imports

// Local Imports
#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use super::manager::Entry;

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use super::{OperationManager, Running};

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use crate::Error;
use crate::Result;

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use crate::plugin::{PluginCaller, PluginGeneration, PluginScope};

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
use crate::operations::OperationDeclaration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
#[derive(Clone)]
pub struct Caller {
    pub(super) operations: OperationManager,
    pub(super) source: PluginCaller,
}

#[derive(Clone)]
pub(super) struct Origin {
    pub(super) identity: Identity,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) scope: Option<PluginScope>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) callback: Option<Weak<Entry>>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) lifecycle: Option<LifecycleLoan>,

    // Calls made by a resident control, which may publish while it retires.
    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) resident: bool,

    // The grant authority admission recorded for the request this origin made.
    pub(super) grants: Option<std::sync::Arc<super::manager::GrantAuthority>>,
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    Plugin(PluginCaller),

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    Binding(Weak<Entry>),

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    Callback(Weak<Entry>),

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    Plugin {
        owner: Arc<str>,
        generation: PluginGeneration,
    },
}

/* === Traits === */

/* === Implementations === */

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
impl OperationManager {
    pub fn caller(&self, source: &PluginCaller) -> Result<Caller> {
        Source::Plugin(source.clone()).check()?;
        Ok(Caller { operations: self.clone(), source: source.clone() })
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
        Self { operations: self.operations.with_context(context), source: self.source.clone() }
    }

    pub fn generation(&self) -> PluginGeneration {
        self.source.generation()
    }

    pub fn submit_encoded(&self, operation: &str, payload: Vec<u8>) -> Result<super::EncodedInvocation> {
        self.operations.submit_encoded_as(operation, payload, Source::Plugin(self.source.clone()), None)
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

    pub async fn call<O: OperationDeclaration>(&self, request: O::Request) -> Result<crate::operations::Response<O::Response>> {
        self.submit::<O>(request)?.await
    }
}

impl Source {
    #[cfg(feature = "grpc")]
    pub(super) fn external(self) -> Result<Self> {
        Ok(Self::External { source: Box::new(self), id: super::handler::fresh_instance()? })
    }

    pub(super) fn check(&self) -> Result<Origin> {
        #[cfg(feature = "grpc")]
        if let Self::External { source, id } = self {
            let mut origin = source.check()?;
            origin.identity = Identity::External(*id);
            return Ok(origin);
        }

        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        let scope = match self {
            Self::Host => None,
            #[cfg(feature = "grpc")]
            Self::External { .. } => unreachable!("external origin was checked"),
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
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            identity: scope
                .as_ref()
                .map_or(Identity::Host, |scope| Identity::Plugin { owner: scope.owner_identity(), generation: scope.generation() }),
            #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
            identity: Identity::Host,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            scope,
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            callback: match self {
                Self::Callback(entry) => Some(entry.clone()),
                _ => None,
            },
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            lifecycle: match self {
                Self::Lifecycle { loan, .. } => Some(loan.clone()),
                _ => None,
            },
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
            return Self::External { source: Box::new(source.for_attempt(entry, _scope, _origin)), id: *id };
        }
        // A Host Provider Serving A Plugin Requirement Acts For That Caller
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if entry.plugin().is_none() && _origin.scope.is_some() {
            return self.clone();
        }
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if _entry.plugin().is_some() {
            return Self::Binding(Arc::downgrade(_entry));
        }
        Self::Host
    }

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) fn for_callback(entry: &Arc<Entry>) -> Self {
        if entry.plugin().is_some() { Self::Callback(Arc::downgrade(entry)) } else { Self::Host }
    }
}

#[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        {
            self.lifecycle.as_ref().is_some_and(LifecycleLoan::is_live)
        }
        #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
        {
            false
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
