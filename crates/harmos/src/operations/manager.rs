/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod discovery;
mod residents;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
// Third Party Imports
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
// Local Imports
use super::admission::{Admission, Lanes};
use super::binding::Typed;
use super::caller::{Identity, Origin, Source};
use super::{Context, OperationGeneration, OperationInstance, OperationInstanceId};

use super::instances::Instances;
use crate::operations::{CacheKey, Definition, OperationDeclaration};
#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
use crate::plugin::PluginScope;
use crate::supervision::executor;
use crate::supervision::instance::Instance;
use crate::{Error, Identifier, Result, operations::Lane, operations::Scope, state::Position};

/* === Constants === */
const DEFAULT_CACHE_CAPACITY: usize = 64;

/* === Macros === */

/* === Type Aliases === */
type CachedResponse = harmos_contract::__private::ErasedResponse;

/* === Structs === */
#[derive(Clone)]
pub struct OperationManager {
    pub(super) registry: Arc<Registry>,
    pub(super) instances: Arc<Instances>,
    pub(super) lanes: Arc<Lanes>,
    pub(super) preparation: Option<Arc<Semaphore>>,
    pub(super) context: Arc<std::sync::OnceLock<super::CallContext>>,
    pub(super) resident: super::resident::Registry,
}

pub(crate) struct Registry {
    middleware: super::middleware::Configuration,
    state: Mutex<State>,
}

#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
pub(crate) struct GenerationCommit<'a> {
    _state: std::sync::MutexGuard<'a, State>,
}

struct State {
    closing: bool,
    entries: BTreeMap<String, BTreeMap<String, Arc<Entry>>>,
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    staging: Vec<std::sync::Weak<PluginScope>>,
    #[cfg(feature = "grpc")]
    grpc: BTreeMap<String, Weak<Entry>>,
    #[cfg(feature = "grpc")]
    grpc_services: std::collections::BTreeSet<String>,
    cache: VecDeque<Cached>,
    cache_capacity: usize,
}

pub(super) struct Entry {
    owner: Identifier,
    definition: Definition,
    pub(super) contract: Option<crate::operations::Contract>,
    #[cfg(feature = "grpc")]
    pub(super) grpc: Option<crate::grpc::routing::Handler>,
    generation: OperationGeneration,
    lane: &'static str,
    bounded: bool,
    // A resident control runs until cancelled, even while its plugin retires.
    #[cfg(feature = "plugin-sidecar")]
    resident: bool,
    active: AtomicBool,
    retirement: AtomicU8,
    pending: Mutex<BTreeMap<OperationInstanceId, Arc<Instance>>>,
    binding: Arc<dyn Any + Send + Sync>,
    #[cfg(feature = "plugin-sidecar")]
    pub(super) native_binding: Option<Arc<dyn Any + Send + Sync>>,
    pub(super) wire: Option<super::wire::Dispatch>,
    #[cfg(feature = "plugin-sidecar")]
    pub(super) native: Option<Arc<super::native::Binding>>,
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    plugin: Option<PluginScope>,
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) sdk: Option<harmos_contract::operations::lifecycle::Action>,
}

pub(super) struct Submission {
    pub(super) submitted: tokio::time::Instant,
    pub(super) entry: Arc<Entry>,
    pub(super) cause: Option<Position>,
    pub(super) preparation: Option<OwnedSemaphorePermit>,
}

// The caller's grant authority for one admitted request. Typed bindings check
// the request grant at admission; encoded bindings learn it from the provider
// while preparing and check it before any attempt runs.
#[derive(Clone, Debug)]
pub(super) struct GrantAuthority {
    owner: String,
    operation: String,
    // A cross-owner plugin caller and its declared requirement for this operation.
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    plugin: Option<(
        String,
        Option<harmos_contract::inventory::contracts::Required>,
    )>,
    // The grants an external caller's authorizer selected for this operation.
    external: Option<harmos_contract::inventory::contracts::Required>,
}

struct Cached {
    target: Weak<Entry>,
    key: CacheKey,
    identity: Identity,
    response: CachedResponse,
}

/* === Enums === */
#[repr(u8)]
enum Retirement {
    Retained,
    Abandoned,
    Explicit,
    Runtime,
}

/* === Traits === */

/* === Implementations === */
impl OperationManager {
    pub(crate) fn new(
        instances: Arc<Instances>,
        settings: &super::Settings,
        reserved: std::collections::BTreeSet<String>,
        identity: Arc<()>,
    ) -> Self {
        Self {
            resident: super::resident::Registry::new(&instances, reserved, identity),
            context: Arc::new(std::sync::OnceLock::from(super::CallContext::default())),
            instances,
            preparation: settings
                .preparation_bytes
                .map(|bytes| Arc::new(Semaphore::new(bytes.get()))),
            lanes: Arc::new(Lanes::default()),
            registry: Arc::new(Registry {
                middleware: super::middleware::Configuration::default(),
                state: Mutex::new(State {
                    closing: false,
                    entries: BTreeMap::new(),
                    #[cfg(any(
                        feature = "plugin",
                        feature = "plugin-guest",
                        feature = "plugin-sidecar"
                    ))]
                    staging: Vec::new(),
                    #[cfg(feature = "grpc")]
                    grpc: BTreeMap::new(),
                    #[cfg(feature = "grpc")]
                    grpc_services: std::collections::BTreeSet::new(),
                    cache: VecDeque::new(),
                    cache_capacity: DEFAULT_CACHE_CAPACITY,
                }),
            }),
        }
    }

    pub fn register_middleware(
        &self,
        middleware: Arc<dyn super::middleware::Middleware>,
    ) -> Result<()> {
        self.registry.middleware.register(middleware)
    }

    pub fn register_authorizer(
        &self,
        authorizer: Arc<dyn super::authorization::Authorizer>,
    ) -> Result<()> {
        self.registry.middleware.register_authorizer(authorizer)
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn submit_external_charged<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: super::StreamItem<O::Request>,
        endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>(
            (super::binding::Request::charged(request)?, endpoints),
            None,
            &[],
            &Source::Host.external()?,
            None,
        )
    }

    pub(super) fn required_external(
        &self,
        entry: &Entry,
        origin: &Origin,
    ) -> Result<Option<harmos_contract::inventory::contracts::Required>> {
        #[cfg(feature = "grpc")]
        if matches!(origin.identity, super::caller::Identity::External(_)) {
            let Some(authorizer) = self.registry.middleware.authorizer() else {
                // Preserve Existing Explicit Anonymous Routes When No Policy Is Configured.
                return Ok(None);
            };
            let denied = || Error::OperationUnauthorized {
                caller: "external".to_owned(),
                owner: entry.owner.to_string(),
                operation: entry.id().to_owned(),
            };
            let contract = entry.contract.as_ref().ok_or_else(denied)?;
            let selected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                authorizer.required(
                    self.call_context(),
                    super::authorization::Provider {
                        owner: &entry.owner,
                        generation: entry.generation,
                        definition: &entry.definition,
                        contract,
                    },
                )
            }))
            .map_err(|_| Error::InvalidInput {
                field: "operation authorizer",
                reason: "authorization callback panicked",
            })??
            .ok_or_else(denied)?;
            if !contract.compatible(&selected.contract) {
                return Err(denied());
            }
            return Ok(Some(selected));
        }
        #[cfg(not(feature = "grpc"))]
        let _ = (entry, origin);
        Ok(None)
    }

    pub fn with_context(&self, context: super::CallContext) -> Self {
        let mut handle = self.clone();
        handle.context = Arc::new(std::sync::OnceLock::from(context));
        handle
    }

    pub(super) fn call_context(&self) -> &super::CallContext {
        self.context
            .get()
            .expect("context freezes before operation execution")
    }

    pub fn status(&self, id: OperationInstanceId) -> Option<super::OperationStatus> {
        self.instances.status(id)
    }

    pub(super) fn claim_preparation(
        &self,
        bytes: std::num::NonZeroUsize,
    ) -> Result<OwnedSemaphorePermit> {
        let count = u32::try_from(bytes.get()).map_err(|_| Error::ResourceExhausted {
            resource: "operation preparation bytes",
        })?;
        self.preparation
            .as_ref()
            .ok_or(Error::InvalidInput {
                field: "operation preparation bytes",
                reason: "encoded preparation requires an explicit preparation budget",
            })?
            .clone()
            .try_acquire_many_owned(count)
            .map_err(|_| Error::ResourceExhausted {
                resource: "operation preparation bytes",
            })
    }

    pub fn register_operation<T>(
        &self,
        owner: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_operation_at::<T>(owner, lane.name(), resources)
    }

    pub fn register_operation_id<T>(
        &self,
        owner: impl Into<String>,
        id: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_operation_id::<T>(id, lane, resources)
        })
    }

    pub(super) fn register_operation_at<T>(
        &self,
        owner: impl Into<String>,
        lane: &'static str,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        self.register_with(owner, |bindings| {
            bindings.register_owned_at::<T>(lane, resources)
        })
    }

    pub fn register_wire_operation<T>(
        &self,
        owner: impl Into<String>,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
        T::Declaration: crate::operations::WireOperation,
        <T::Declaration as OperationDeclaration>::Request:
            crate::Message + crate::schema::Described,
        <T::Declaration as OperationDeclaration>::Response:
            crate::Message + crate::schema::Described,
    {
        self.register_with(owner, |bindings| {
            bindings.register_wire_operation::<T>(lane, resources)
        })
    }

    pub fn register_wire_contract<T>(
        &self,
        owner: impl Into<String>,
        contract: &crate::operations::Contract,
        lane: impl Lane,
        resources: Arc<T::Resources>,
    ) -> Result<()>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
        T::Declaration: crate::operations::WireOperation,
        <T::Declaration as OperationDeclaration>::Request:
            crate::Message + crate::schema::Described,
        <T::Declaration as OperationDeclaration>::Response:
            crate::Message + crate::schema::Described,
    {
        self.register_with(owner, |bindings| {
            bindings.register_wire_contract::<T>(contract, lane, resources)
        })
    }

    pub(super) fn binding_owned_definition<T>(
        &self,
        owner: impl Into<String>,
        lane: &'static str,
        resources: Arc<T::Resources>,
        definition: Definition,
        wire: Option<super::wire::Dispatch>,
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>>
    where
        T: for<'ctx> super::Operation<Context<'ctx> = Context<'ctx>>,
    {
        definition
            .validate()
            .map_err(|source| Error::OperationDeclaration { source })?;
        if <T::Endpoints as super::EndpointSet>::item_types() != T::Declaration::endpoint_types() {
            return Err(Error::InvalidInput {
                field: "operation endpoint types",
                reason: "authored endpoints do not match declared stream item types",
            });
        }
        let binding = super::authored::factory::<T>(self, resources);
        self.binding_typed(
            owner,
            lane,
            binding,
            definition,
            wire,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            plugin,
        )
    }

    fn binding_typed<O: OperationDeclaration, P: Send + 'static>(
        &self,
        owner: impl Into<String>,
        lane: &'static str,
        binding: Typed<O, P>,
        definition: Definition,
        wire: Option<super::wire::Dispatch>,
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        plugin: Option<PluginScope>,
    ) -> Result<Arc<Entry>> {
        let owner =
            Identifier::new(owner.into()).map_err(|source| Error::OperationDeclaration {
                source: source.into(),
            })?;
        let bounded = self
            .instances
            .lane_bounded(lane)
            .ok_or_else(|| Error::NotRegistered {
                domain: "operation lane",
                name: lane.to_owned(),
            })?;
        let generation = OperationGeneration::fresh()?;
        Ok(Arc::new(Entry {
            owner,
            definition,
            contract: None,
            #[cfg(feature = "grpc")]
            grpc: None,
            generation,
            lane,
            bounded,
            #[cfg(feature = "plugin-sidecar")]
            resident: false,
            active: AtomicBool::new(true),
            retirement: AtomicU8::new(Retirement::Retained as u8),
            pending: Mutex::new(BTreeMap::new()),
            binding: Arc::new(binding),
            #[cfg(feature = "plugin-sidecar")]
            native_binding: None,
            wire,
            #[cfg(feature = "plugin-sidecar")]
            native: None,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            plugin,
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            sdk: None,
        }))
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub fn validate_requirements(
        &self,
        candidate: &crate::plugin::PluginDefinitions,
    ) -> Result<()> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        for (definition, required) in candidate.host_requirements() {
            let id = required.contract.operation().id.as_str();
            let selected =
                state
                    .host_provider(id, &required.contract)
                    .map_err(|error| match error {
                        Error::NotRegistered { domain, .. } => Error::NotRegistered {
                            domain,
                            name: format!("{}/{id}", definition.id),
                        },
                        other => other,
                    })?;
            selected.available()?;
        }
        Ok(())
    }

    pub(super) fn reserve(&self, entries: &[Arc<Entry>]) -> Result<()> {
        #[cfg(feature = "grpc")]
        for entry in entries {
            let Some(contract) = &entry.contract else {
                continue;
            };
            let Some(binding) = contract.grpc() else {
                continue;
            };
            crate::grpc::descriptor(
                &binding.package,
                &binding.service,
                &[(&binding.method, contract)],
            )
            .map_err(|source| Error::OperationFailed {
                operation: entry.id().to_owned(),
                source: Arc::new(source),
            })?;
        }
        let (result, retired) = {
            let mut state = self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy");
            let retired = state.retired();
            let result = if state.closing || !self.instances.is_accepting() {
                Err(Error::Closing {
                    domain: "operations",
                })
            } else if let Some(entry) = entries
                .iter()
                .find(|entry| state.owned(entry.id(), entry.owner.as_str()).is_some())
            {
                Err(Error::AlreadyRegistered {
                    domain: "operation",
                    name: entry.id().to_owned(),
                })
            } else {
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                let gate = entries
                    .iter()
                    .find_map(|entry| entry.plugin.as_ref())
                    .map_or(Ok(()), |scope| {
                        if scope.is_closed() {
                            Err(Error::Closing {
                                domain: "plugin scope",
                            })
                        } else if scope.has_committed() {
                            Err(Error::InvalidInput {
                                field: "operation catalog",
                                reason: "plugin generation is already committed",
                            })
                        } else {
                            Ok(())
                        }
                    });
                #[cfg(not(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                )))]
                let gate: Result<()> = Ok(());
                #[cfg(feature = "grpc")]
                let gate = gate.and_then(|()| {
                    let mut staged = BTreeMap::new();
                    for entry in entries {
                        let Some(binding) =
                            entry.contract.as_ref().and_then(|contract| contract.grpc())
                        else {
                            continue;
                        };
                        let path = binding.path();
                        if state
                            .grpc_services
                            .contains(&format!("{}.{}", binding.package, binding.service))
                            || state.grpc.get(&path).and_then(Weak::upgrade).is_some()
                            || staged.insert(path.clone(), Arc::downgrade(entry)).is_some()
                        {
                            return Err(Error::AlreadyRegistered {
                                domain: "gRPC method",
                                name: path,
                            });
                        }
                    }
                    state.grpc.extend(staged);
                    Ok(())
                });
                if gate.is_ok() {
                    for entry in entries {
                        state
                            .entries
                            .entry(entry.id().to_owned())
                            .or_default()
                            .insert(entry.owner.to_string(), Arc::clone(entry));
                    }
                }
                gate
            };
            (result, retired)
        };
        // Resource owners removed by opportunistic retirement never drop under a registry guard.
        drop(retired);
        result
    }

    pub fn definition(&self, id: &str) -> Result<Definition> {
        self.entry(id).map(|entry| entry.definition.clone())
    }

    pub fn definition_from(&self, owner: &str, id: &str) -> Result<Definition> {
        let entry = self.capture_provider(id, Some(owner))?;
        entry.available()?;
        Ok(entry.definition.clone())
    }

    pub fn submit_from<O: OperationDeclaration>(
        &self,
        owner: &str,
        request: O::Request,
    ) -> Result<OperationInstance<O>> {
        let submitted = executor::now();
        let entry = self.capture_provider(O::ID, Some(owner))?;
        self.admit_entry::<O, ()>(
            (request.into(), ()),
            None,
            &[],
            &Source::Host,
            Submission {
                submitted,
                entry,
                cause: None,
                preparation: None,
            },
        )
    }

    pub fn definitions(&self) -> Vec<Definition> {
        self.registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .definitions()
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn reserve_grpc_service(&self, name: &str) -> Result<()> {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        let prefix = format!("/{name}/");
        if state.grpc_services.contains(name)
            || state.grpc.keys().any(|path| path.starts_with(&prefix))
        {
            return Err(Error::AlreadyRegistered {
                domain: "gRPC service",
                name: name.to_owned(),
            });
        }
        state.grpc_services.insert(name.to_owned());
        Ok(())
    }

    #[cfg(feature = "grpc")]
    pub fn grpc_methods(&self) -> Vec<super::GrpcMethod> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Vec::new();
        }
        state
            .grpc
            .values()
            .filter_map(Weak::upgrade)
            .filter(|entry| entry.active.load(Ordering::Acquire) && entry.available().is_ok())
            .filter_map(|entry| {
                Some(super::GrpcMethod {
                    owner: entry.owner.clone(),
                    generation: entry.generation,
                    contract: entry.contract.clone()?,
                })
            })
            .collect()
    }

    #[cfg(feature = "grpc")]
    pub(crate) fn grpc_binding(&self, path: &str) -> Result<super::GrpcBinding> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        let entry = state
            .grpc
            .get(path)
            .and_then(Weak::upgrade)
            .ok_or_else(|| Error::NotRegistered {
                domain: "gRPC method",
                name: path.to_owned(),
            })?;
        if !entry.active.load(Ordering::Acquire) {
            return Err(Error::Closing {
                domain: "operation",
            });
        }
        entry.available()?;
        Ok(super::GrpcBinding {
            operations: self.clone(),
            entry,
        })
    }

    pub fn owner(&self, id: &str) -> Result<Identifier> {
        self.entry(id).map(|entry| entry.owner.clone())
    }

    // The declaration is its own request; `submit_as::<O>` names a separate one.
    pub fn submit<O: OperationDeclaration<Request = O>>(
        &self,
        request: O,
    ) -> Result<OperationInstance<O>> {
        self.submit_as::<O>(request)
    }

    pub async fn call<O: OperationDeclaration<Request = O>>(
        &self,
        request: O,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.call_as::<O>(request).await
    }
    pub fn submit_as<O: OperationDeclaration>(
        &self,
        request: O::Request,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, ()>((request.into(), ()), None, &[], &Source::Host, None)
    }

    pub fn submit_endpoints<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: O::Request,
        endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>((request.into(), endpoints), None, &[], &Source::Host, None)
    }

    pub fn submit_charged<O: OperationDeclaration, P: Send + 'static>(
        &self,
        request: super::StreamItem<O::Request>,
        endpoints: P,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>(
            (super::binding::Request::charged(request)?, endpoints),
            None,
            &[],
            &Source::Host,
            None,
        )
    }

    pub fn submit_at<O: OperationDeclaration>(
        &self,
        id: &str,
        request: O::Request,
    ) -> Result<OperationInstance<O>> {
        self.admit_at::<O, ()>(id, (request.into(), ()), None, &[], &Source::Host, None)
    }

    pub async fn call_as<O: OperationDeclaration>(
        &self,
        request: O::Request,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.submit_as::<O>(request)?.await
    }

    pub fn submit_caused<O: OperationDeclaration>(
        &self,
        request: O::Request,
        cause: Position,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, ()>((request.into(), ()), None, &[], &Source::Host, Some(cause))
    }

    pub async fn call_caused<O: OperationDeclaration>(
        &self,
        request: O::Request,
        cause: Position,
    ) -> Result<crate::operations::Response<O::Response>> {
        self.submit_caused::<O>(request, cause)?.await
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn submit_by<O: OperationDeclaration, P: Send + 'static>(
        &self,
        input: (super::binding::Request<O::Request>, P),
        source: &Source,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>(input, None, &[], source, None)
    }

    pub(super) fn submit_scoped<O: OperationDeclaration, P: Send + 'static>(
        &self,
        input: (super::binding::Request<O::Request>, P),
        scope: &Scope<'_>,
        admission: &Admission,
        source: &Source,
        cause: Option<Position>,
    ) -> Result<OperationInstance<O>> {
        self.admit::<O, P>(input, Some(scope), admission.held(), source, cause)
    }

    fn admit<O: OperationDeclaration, P: Send + 'static>(
        &self,
        input: (super::binding::Request<O::Request>, P),
        scope: Option<&Scope<'_>>,
        ancestors: &[&'static str],
        source: &Source,
        cause: Option<Position>,
    ) -> Result<OperationInstance<O>> {
        self.admit_at::<O, P>(O::ID, input, scope, ancestors, source, cause)
    }

    pub(super) fn admit_at<O: OperationDeclaration, P: Send + 'static>(
        &self,
        id: &str,
        input: (super::binding::Request<O::Request>, P),
        scope: Option<&Scope<'_>>,
        ancestors: &[&'static str],
        source: &Source,
        cause: Option<Position>,
    ) -> Result<OperationInstance<O>> {
        let submitted = executor::now();
        let origin = source.check()?;
        let entry = self.entry_as(id, &origin)?;
        self.admit_entry::<O, P>(
            input,
            scope,
            ancestors,
            source,
            Submission {
                submitted,
                entry,
                cause,
                preparation: None,
            },
        )
    }

    pub(super) fn admit_entry<O: OperationDeclaration, P: Send + 'static>(
        &self,
        (mut request, endpoints): (super::binding::Request<O::Request>, P),
        scope: Option<&Scope<'_>>,
        ancestors: &[&'static str],
        source: &Source,
        submission: Submission,
    ) -> Result<OperationInstance<O>> {
        let Submission {
            submitted,
            entry,
            cause,
            preparation,
        } = submission;
        let origin = source.check()?;
        if self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .closing
            && !entry.retirement_control()
            && !(scope.is_some() && origin.has_lifecycle_loan())
        {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        entry.available_to(&origin)?;
        entry.authorize(&origin)?;
        let required = self.required_external(&entry, &origin)?;
        let authority = entry.grant_authority(&origin, required);
        authority.admit(O::grant(&request.value).as_ref())?;
        // Encoded bindings carry the authority to the provider's prepared grant.
        let origin = Origin {
            grants: Some(Arc::new(authority)),
            ..origin
        };
        // Authoring Callbacks Run Outside Registry/Pending Locks. Recheck The Provider.
        source.check()?;
        entry.available_to(&origin)?;
        let binding = Arc::clone(&entry.binding).downcast::<Typed<O, P>>();
        #[cfg(feature = "plugin-sidecar")]
        let binding = binding.or_else(|original| {
            entry
                .native_binding
                .as_ref()
                .map_or(Err(original), |encoded| {
                    Arc::clone(encoded).downcast::<Typed<O, P>>()
                })
        });
        let binding = binding.map_err(|_| Error::InvalidInput {
            field: "operation",
            reason: "identity belongs to a different operation or endpoint type",
        })?;
        let admission = self.lanes.admit(ancestors, entry.lane, entry.bounded)?;
        let id = super::handler::fresh_instance()?;
        let mut pending = entry
            .pending
            .lock()
            .expect("operation settlement lock is healthy");
        source.check()?;
        entry.available_to(&origin)?;
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if entry.sdk.is_some() && pending.values().any(|control| !control.is_settled()) {
            return Err(Error::ResourceExhausted {
                resource: "SDK physical settlement",
            });
        }
        let privileged = entry.lifecycle_control() || origin.has_lifecycle_loan();
        let owner = Arc::downgrade(&entry);
        let retired = move || {
            if let Some(entry) = owner.upgrade() {
                let removed = entry
                    .pending
                    .lock()
                    .expect("operation settlement lock is healthy")
                    .remove(&id);
                drop(removed);
            }
        };
        let reservation = match scope {
            Some(scope) => scope.reserve_tracked(id, entry.lane, submitted, privileged, retired)?,
            None => self
                .instances
                .reserve(id, entry.lane, submitted, privileged, retired)?,
        };
        let control = reservation.control();
        self.instances.assign(id, origin.identity.clone());
        pending.insert(id, Arc::clone(&control));
        drop(pending);

        let middleware = self.registry.middleware.capture();
        let context = if middleware.is_empty() {
            self.context.clone()
        } else {
            Arc::new(std::sync::OnceLock::new())
        };
        request.context = context.clone();
        let mut job = (binding.prepare)(
            (request, endpoints),
            id,
            Arc::clone(&control),
            Arc::clone(&entry),
            admission,
            source.clone(),
            origin.clone(),
            preparation,
        )?;
        #[cfg(feature = "grpc")]
        if let Some(deadline) = self.call_context().get::<super::context::Deadline>() {
            job.policy.timeout = job
                .policy
                .timeout
                .min(deadline.0.saturating_duration_since(submitted));
        }
        if !middleware.is_empty() {
            job.middleware = Some(super::middleware::Session::new(
                middleware,
                entry.definition.clone(),
                id,
                entry.generation,
                self.call_context().clone(),
                context,
                origin,
            ));
        }
        let refusal = if executor::now().saturating_duration_since(submitted)
            >= entry.definition.policy.timeout
        {
            Err(crate::Refusal::TimedOut.into())
        } else {
            source
                .check()
                .and_then(|origin| entry.available_to(&origin))
                .and_then(|_| {
                    if self.instances.is_accepting() {
                        Ok(())
                    } else {
                        Err(Error::Closing {
                            domain: "operation",
                        })
                    }
                })
        };
        if let Err(error) = refusal {
            job.refusal = Some(error);
        }
        let outcome = reservation.submit(job, cause);
        Ok(OperationInstance {
            id,
            generation: entry.generation,
            outcome: super::handler::Outcome::Request(outcome),
            control,
            released: false,
        })
    }

    pub(super) fn entry(&self, id: &str) -> Result<Arc<Entry>> {
        let entry = self.capture_entry(id)?;
        entry.available()?;
        Ok(entry)
    }

    pub(super) fn entry_as(&self, id: &str, _origin: &Origin) -> Result<Arc<Entry>> {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some(scope) = &_origin.scope {
            if let Some(required) = Identifier::new(id)
                .ok()
                .and_then(|identifier| scope.requirement(&identifier).cloned())
                .filter(|_| scope.required_provider(id).is_none())
            {
                let entry = {
                    let state = self
                        .registry
                        .state
                        .lock()
                        .expect("operation registry lock is healthy");
                    Arc::clone(state.host_provider(id, &required.contract)?)
                };
                entry.available_to(_origin)?;
                return Ok(entry);
            }
            if let Some(provider) = scope.required_provider(id) {
                let entry = if _origin.has_lifecycle_loan() {
                    let state = self
                        .registry
                        .state
                        .lock()
                        .expect("operation registry lock is healthy");
                    Arc::clone(state.select(id, Some(provider))?)
                } else {
                    self.capture_provider(id, Some(provider))?
                };
                entry.available_to(_origin)?;
                return Ok(entry);
            }
            let owned = self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy")
                .owned(id, scope.owner())
                .cloned();
            if let Some(entry) = owned {
                entry.available_to(_origin)?;
                return Ok(entry);
            }
        }
        self.entry(id)
    }

    // Capture Does Not Grant Readiness; Submission Rechecks The Exact Entry.
    pub(super) fn capture_entry(&self, id: &str) -> Result<Arc<Entry>> {
        self.capture_provider(id, None)
    }

    pub(super) fn capture_provider(&self, id: &str, owner: Option<&str>) -> Result<Arc<Entry>> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        let entry = Some(state.select(id, owner)?)
            .filter(|entry| {
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                if entry.sdk.is_some() {
                    return false;
                }
                entry.active.load(Ordering::Acquire)
            })
            .cloned()
            .ok_or_else(|| Error::NotRegistered {
                domain: "operation",
                name: id.to_owned(),
            })?;
        Ok(entry)
    }

    pub async fn unregister(&self, id: &str) -> Result<()> {
        self.unregister_selected(id, None).await
    }

    pub async fn unregister_from(&self, owner: &str, id: &str) -> Result<()> {
        self.unregister_selected(id, Some(owner)).await
    }

    async fn unregister_selected(&self, id: &str, owner: Option<&str>) -> Result<()> {
        let entry = {
            let state = self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy");
            Arc::clone(state.select(id, owner)?)
        };
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some(scope) = &entry.plugin {
            // Notify the staging gate before revocation, outside registry guards.
            scope.unbind_operations();
        }
        let retired = {
            let mut state = self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy");
            if !state
                .owned(id, entry.owner.as_str())
                .is_some_and(|current| Arc::ptr_eq(current, &entry))
            {
                return Err(Error::NotRegistered {
                    domain: "operation",
                    name: id.to_owned(),
                });
            }
            let _pending = entry
                .pending
                .lock()
                .expect("operation settlement lock is healthy");
            entry
                .retirement
                .store(Retirement::Explicit as u8, Ordering::Release);
            entry.active.store(false, Ordering::Release);
            let retired = state.remove_cache(|cached| {
                !cached
                    .target
                    .upgrade()
                    .is_some_and(|target| !Arc::ptr_eq(&target, &entry))
            });
            drop(_pending);
            retired
        };
        drop(retired);
        settle(&entry).await?;
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state
            .owned(id, entry.owner.as_str())
            .is_some_and(|current| Arc::ptr_eq(current, &entry))
        {
            state.remove_owned(id, entry.owner.as_str());
        }
        Ok(())
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn fence(&self, entries: &[Arc<Entry>], abandoned: bool) {
        let retired = {
            let mut state = self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy");
            for entry in entries {
                let _pending = entry
                    .pending
                    .lock()
                    .expect("operation settlement lock is healthy");
                // A by-name unregister retains its reservation for retry even
                // when the plugin receipt is abandoned during that wait.
                let _ = entry
                    .retirement
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                        (state != Retirement::Explicit as u8).then_some(if abandoned {
                            Retirement::Abandoned as u8
                        } else {
                            Retirement::Retained as u8
                        })
                    });
                entry.active.store(false, Ordering::Release);
            }
            state.remove_cache(|cached| {
                cached
                    .target
                    .upgrade()
                    .is_some_and(|target| entries.iter().any(|entry| Arc::ptr_eq(&target, entry)))
            })
        };
        drop(retired);
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn remove_settled(&self, entries: &[Arc<Entry>]) {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        for entry in entries {
            if entry.settled()
                && state
                    .owned(entry.id(), entry.owner.as_str())
                    .is_some_and(|current| Arc::ptr_eq(current, entry))
            {
                state.remove_owned(entry.id(), entry.owner.as_str());
            }
        }
        // The caller owns every entry until after this guard is released.
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) async fn settle_entries(&self, entries: &[Arc<Entry>]) -> Result<()> {
        self.settle_owned_entries(entries).await?;
        self.remove_settled(entries);
        Ok(())
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) async fn settle_owned_entries(&self, entries: &[Arc<Entry>]) -> Result<()> {
        for entry in entries {
            settle(entry).await?;
        }
        Ok(())
    }

    pub fn cache_capacity(&self) -> usize {
        self.registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .cache_capacity
    }

    pub fn set_cache_capacity(&self, capacity: usize) {
        let retired = {
            let mut state = self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy");
            state.cache_capacity = capacity;
            state.evict_to(capacity)
        };
        drop(retired);
    }

    pub fn clear_cache(&self) {
        let retired = std::mem::take(
            &mut self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy")
                .cache,
        );
        drop(retired);
    }

    pub fn invalidate<O: OperationDeclaration>(&self, key: &CacheKey) -> Result<()> {
        let entry = self.entry(O::ID)?;
        self.invalidate_entry(&entry, key);
        Ok(())
    }

    pub fn invalidate_from<O: OperationDeclaration>(
        &self,
        owner: &str,
        key: &CacheKey,
    ) -> Result<()> {
        let entry = self.capture_provider(O::ID, Some(owner))?;
        entry.available()?;
        self.invalidate_entry(&entry, key);
        Ok(())
    }

    fn invalidate_entry(&self, entry: &Arc<Entry>, key: &CacheKey) {
        let retired = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .remove_cache(|cached| {
                cached.key == *key
                    && cached
                        .target
                        .upgrade()
                        .is_some_and(|target| Arc::ptr_eq(&target, entry))
            });
        drop(retired);
    }

    pub(super) fn lookup<O: OperationDeclaration>(
        &self,
        entry: &Arc<Entry>,
        key: &CacheKey,
        identity: &Identity,
    ) -> Option<crate::operations::Response<O::Response>> {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing || entry.available().is_err() {
            return None;
        }
        let index = state.cache.iter().position(|cached| {
            cached.key == *key
                && cached.identity == *identity
                && cached.target.upgrade().is_some_and(|target| {
                    Arc::ptr_eq(&target, entry) && target.generation == entry.generation
                })
        })?;
        let cached = state.cache.remove(index)?;
        let response = cached.response.clone().restore::<O::Response>();
        state.cache.push_back(cached);
        response
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn track_staging(&self, scope: &Arc<PluginScope>) -> Result<()> {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing {
                domain: "plugin operation staging",
            });
        }
        scope.bind_operation_registry(&self.registry)?;
        state.staging.retain(|scope| scope.strong_count() != 0);
        state.staging.push(Arc::downgrade(scope));
        Ok(())
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn validate_staging(&self, entries: &[Arc<Entry>]) -> Result<()> {
        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing
            || !self.instances.is_accepting()
            || entries.iter().any(|entry| {
                !entry.active.load(Ordering::Acquire)
                    || !state
                        .owned(entry.id(), entry.owner.as_str())
                        .is_some_and(|original| Arc::ptr_eq(original, entry))
            })
        {
            return Err(Error::Closing {
                domain: "plugin operation staging",
            });
        }
        Ok(())
    }

    pub(crate) fn close_admission(&self) {
        self.close_entries(true);
    }

    pub(crate) fn close(&self) {
        self.close_entries(false);
    }

    fn close_entries(&self, _retain_shutdown: bool) {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if _retain_shutdown && state.closing {
            return;
        }
        state.closing = true;
        let retired = std::mem::take(&mut state.cache);
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        let staging: Vec<_> = state
            .staging
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .collect();
        for entry in state
            .entries
            .values()
            .flat_map(|providers| providers.values())
        {
            let _pending = entry
                .pending
                .lock()
                .expect("operation settlement lock is healthy");
            if _retain_shutdown && entry.retirement_control() {
                continue;
            }
            if _retain_shutdown && entry.active.load(Ordering::Acquire) {
                entry
                    .retirement
                    .store(Retirement::Runtime as u8, Ordering::Release);
            }
            entry.active.store(false, Ordering::Release);
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            let ordinary = entry.sdk.is_none();
            #[cfg(not(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            )))]
            let ordinary = true;
            if _retain_shutdown && ordinary {
                for control in _pending.values() {
                    control.cancel();
                }
            }
        }
        drop(state);
        drop(retired);
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        for scope in staging {
            scope.close_staging();
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.close();
        self.instances.close();
        let entries: Vec<_> = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy")
            .entries
            .values()
            .flat_map(|providers| providers.values())
            .cloned()
            .collect();
        for entry in entries {
            settle(&entry).await?;
        }
        self.instances.settle().await?;
        let retired = std::mem::take(
            &mut self
                .registry
                .state
                .lock()
                .expect("operation registry lock is healthy")
                .entries,
        );
        drop(retired);
        Ok(())
    }
}

impl Entry {
    // Move a resident control onto the unbounded resident lane.
    #[cfg(feature = "plugin-sidecar")]
    pub(super) fn resident(&mut self) {
        self.lane = super::lane::RESIDENT;
        self.bounded = false;
        self.resident = true;
    }

    // A resident control and its children outlive the committed plugin's
    // admission until the host cancels them.
    pub(super) fn retires_late(&self) -> bool {
        #[cfg(feature = "plugin-sidecar")]
        if self.resident
            && self
                .plugin
                .as_ref()
                .is_some_and(|scope| scope.has_committed())
        {
            return true;
        }
        false
    }

    pub(super) fn declared(&self) -> &Definition {
        &self.definition
    }

    pub(super) fn id(&self) -> &str {
        self.definition.id.as_str()
    }

    fn lifecycle_control(&self) -> bool {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if self.sdk.is_some() {
            return true;
        }
        self.retirement_control()
    }

    pub(super) fn retirement_control(&self) -> bool {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if self.sdk == Some(harmos_contract::operations::lifecycle::Action::Shutdown) {
            return true;
        }
        false
    }

    pub(super) fn available_to(&self, origin: &Origin) -> Result<()> {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if origin.has_lifecycle_loan() {
            return self.available_for_lifecycle();
        }
        let _ = origin;
        self.available()
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn admitted_shutdown(&self, control: &Arc<Instance>) -> bool {
        self.sdk == Some(harmos_contract::operations::lifecycle::Action::Shutdown)
            && self.active.load(Ordering::Acquire)
            && !control.is_settled()
            && self
                .pending
                .lock()
                .expect("operation settlement lock is healthy")
                .values()
                .any(|original| Arc::ptr_eq(original, control))
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn available_for_lifecycle(&self) -> Result<()> {
        if self.sdk.is_some() || self.active.load(Ordering::Acquire) {
            return self.available();
        }
        if self.retirement.load(Ordering::Acquire) != Retirement::Runtime as u8 {
            return Err(Error::Closing {
                domain: "operation",
            });
        }
        if self
            .plugin
            .as_ref()
            .is_some_and(|scope| !scope.has_committed())
        {
            return Err(Error::NotReady {
                domain: "operation plugin",
            });
        }
        Ok(())
    }

    pub(super) fn available(&self) -> Result<()> {
        let retiring = self.retires_late();
        if !self.active.load(Ordering::Acquire) && !retiring {
            return Err(Error::Closing {
                domain: "operation",
            });
        }
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some(scope) = &self.plugin {
            if let Some(action) = self.sdk {
                use harmos_contract::operations::lifecycle::Action;
                return match action {
                    Action::Initialize if scope.is_closed() => Err(Error::Closing {
                        domain: "SDK initialization",
                    }),
                    Action::Initialize if scope.has_committed() => Err(Error::InvalidInput {
                        field: "SDK initialization",
                        reason: "generation is already committed",
                    }),
                    Action::Initialize | Action::Shutdown => Ok(()),
                };
            }
            if scope.is_closed() && !retiring {
                return Err(Error::Closing {
                    domain: "operation plugin",
                });
            }
            if !scope.is_ready() && !retiring {
                return Err(Error::NotReady {
                    domain: "operation plugin",
                });
            }
        }
        Ok(())
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(super) fn plugin(&self) -> Option<&PluginScope> {
        self.plugin.as_ref()
    }

    fn grant_authority(
        &self,
        _origin: &Origin,
        external: Option<harmos_contract::inventory::contracts::Required>,
    ) -> GrantAuthority {
        GrantAuthority {
            owner: self.owner.to_string(),
            operation: self.id().to_owned(),
            #[cfg(any(
                feature = "plugin",
                feature = "plugin-guest",
                feature = "plugin-sidecar"
            ))]
            plugin: _origin
                .scope
                .as_ref()
                .filter(|caller| caller.owner() != self.owner.as_str())
                .map(|caller| {
                    (
                        caller.owner().to_owned(),
                        caller.requirement(&self.definition.id).cloned(),
                    )
                }),
            external,
        }
    }

    pub(super) fn authorize(&self, _origin: &Origin) -> Result<()> {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some(caller) = &_origin.scope {
            if self.sdk.is_some() {
                return Err(Error::OperationUnauthorized {
                    caller: caller.owner().to_owned(),
                    owner: self.owner.to_string(),
                    operation: self.id().to_owned(),
                });
            }
            if self.definition.visibility == crate::operations::Visibility::Host {
                return Err(Error::OperationUnauthorized {
                    caller: caller.owner().to_owned(),
                    owner: self.owner.to_string(),
                    operation: self.id().to_owned(),
                });
            }
            let own = caller.owner() == self.owner.as_str()
                && self
                    .plugin
                    .as_ref()
                    .is_some_and(|scope| scope.generation() == caller.generation())
                && (self.definition.visibility == crate::operations::Visibility::Private
                    || _origin
                        .callback
                        .as_ref()
                        .is_some_and(|entry| std::ptr::eq(entry.as_ptr(), self)));
            let dependency = caller.owner() != self.owner.as_str()
                && self.definition.visibility == crate::operations::Visibility::Shared
                && caller.allows_operation(
                    self.owner.as_str(),
                    self.contract.as_ref(),
                    &self.definition.id,
                    self.plugin.is_none(),
                );
            if !own && !dependency {
                return Err(Error::OperationUnauthorized {
                    caller: caller.owner().to_owned(),
                    owner: self.owner.to_string(),
                    operation: self.id().to_owned(),
                });
            }
        }
        Ok(())
    }

    pub(super) fn settled(&self) -> bool {
        self.pending
            .lock()
            .expect("operation settlement lock is healthy")
            .values()
            .all(|settled| settled.is_settled())
    }
}

impl Registry {
    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    pub(crate) fn generation_commit(&self) -> Result<GenerationCommit<'_>> {
        let state = self
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing {
            return Err(Error::Closing {
                domain: "plugin generation commit",
            });
        }
        Ok(GenerationCommit { _state: state })
    }

    pub(super) fn insert<O: OperationDeclaration>(
        &self,
        entry: &Arc<Entry>,
        key: CacheKey,
        identity: Identity,
        response: crate::operations::Response<O::Response>,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing
            || state.cache_capacity == 0
            || entry.available().is_err()
            || !state
                .owned(entry.id(), entry.owner.as_str())
                .is_some_and(|current| Arc::ptr_eq(current, entry))
        {
            return;
        }
        let mut retired = state.remove_cache(|cached| {
            cached.key == key
                && cached.identity == identity
                && cached
                    .target
                    .upgrade()
                    .is_some_and(|target| Arc::ptr_eq(&target, entry))
        });
        let available = state.cache_capacity - 1;
        retired.extend(state.evict_to(available));
        state.cache.push_back(Cached {
            target: Arc::downgrade(entry),
            key,
            identity,
            response: response.into_erased(),
        });
        drop(state);
        drop(retired);
    }
}

impl State {
    fn owned(&self, id: &str, owner: &str) -> Option<&Arc<Entry>> {
        self.entries.get(id)?.get(owner)
    }

    #[cfg(any(
        feature = "plugin",
        feature = "plugin-guest",
        feature = "plugin-sidecar"
    ))]
    fn host_provider(
        &self,
        id: &str,
        contract: &crate::operations::Contract,
    ) -> Result<&Arc<Entry>> {
        let mut compatible = self
            .entries
            .get(id)
            .into_iter()
            .flat_map(|providers| providers.values())
            .filter(|entry| entry.plugin.is_none() && entry.contract.as_ref() == Some(contract));
        let selected = compatible.next().ok_or_else(|| Error::NotRegistered {
            domain: "host provider",
            name: id.to_owned(),
        })?;
        if compatible.next().is_some() {
            return Err(Error::InvalidInput {
                field: "host provider",
                reason: "required contract has multiple host providers",
            });
        }
        Ok(selected)
    }

    fn select(&self, id: &str, owner: Option<&str>) -> Result<&Arc<Entry>> {
        let providers = self.entries.get(id).ok_or_else(|| Error::NotRegistered {
            domain: "operation",
            name: id.to_owned(),
        })?;
        if let Some(owner) = owner {
            return providers.get(owner).ok_or_else(|| Error::NotRegistered {
                domain: "operation provider",
                name: format!("{owner}/{id}"),
            });
        }
        if providers.len() != 1 {
            return Err(Error::InvalidInput {
                field: "operation provider",
                reason: "operation has multiple providers; an explicit provider is required",
            });
        }
        Ok(providers.values().next().expect("one original provider"))
    }

    fn remove_owned(&mut self, id: &str, owner: &str) -> Option<Arc<Entry>> {
        let providers = self.entries.get_mut(id)?;
        let removed = providers.remove(owner);
        #[cfg(feature = "grpc")]
        if let Some(binding) = removed
            .as_ref()
            .and_then(|entry| entry.contract.as_ref())
            .and_then(|contract| contract.grpc())
        {
            self.grpc.remove(&binding.path());
        }
        if providers.is_empty() {
            self.entries.remove(id);
        }
        removed
    }

    fn retired(&mut self) -> Vec<Arc<Entry>> {
        let names: Vec<_> = self
            .entries
            .iter()
            .flat_map(|(id, providers)| {
                providers
                    .iter()
                    .map(move |(owner, entry)| (id, owner, entry))
            })
            .filter(|(_, _, entry)| {
                entry.retirement.load(Ordering::Acquire) == Retirement::Abandoned as u8
                    && entry.settled()
            })
            .map(|(id, owner, _)| (id.clone(), owner.clone()))
            .collect();
        names
            .into_iter()
            .filter_map(|(id, owner)| self.remove_owned(&id, &owner))
            .collect()
    }

    fn definitions(&self) -> Vec<Definition> {
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        let mut ready = BTreeMap::new();
        self.entries
            .values()
            .flat_map(|providers| providers.values())
            .filter(|entry| {
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                if entry.sdk.is_some() {
                    return false;
                }
                if !entry.active.load(Ordering::Acquire) {
                    return false;
                }
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                if let Some(scope) = &entry.plugin {
                    return *ready
                        .entry(scope.generation())
                        .or_insert_with(|| scope.is_ready());
                }
                true
            })
            .map(|entry| entry.definition.clone())
            .collect()
    }

    fn remove_cache(&mut self, mut remove: impl FnMut(&Cached) -> bool) -> Vec<Cached> {
        let mut kept = VecDeque::new();
        let mut retired = Vec::new();
        for cached in std::mem::take(&mut self.cache) {
            if remove(&cached) {
                retired.push(cached);
            } else {
                kept.push_back(cached);
            }
        }
        self.cache = kept;
        retired
    }

    fn evict_to(&mut self, capacity: usize) -> Vec<Cached> {
        let mut retired = Vec::new();
        while self.cache.len() > capacity {
            if let Some(cached) = self.cache.pop_front() {
                retired.push(cached);
            }
        }
        retired
    }
}

impl GrantAuthority {
    // Refuse a request whose grant the caller does not hold. A request resolves
    // only to a grant on the operation it calls; a provider preparing a grant on
    // another operation is refused for every caller.
    pub(super) fn admit(&self, grant: Option<&harmos_contract::plugin::Grant>) -> Result<()> {
        let denied = |caller: &str| Error::OperationUnauthorized {
            caller: caller.to_owned(),
            owner: self.owner.clone(),
            operation: self.operation.clone(),
        };
        if grant.is_some_and(|grant| grant.operation() != self.operation) {
            return Err(Error::PermissionDenied);
        }
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some((caller, required)) = &self.plugin
            && !grant_allowed(required.as_ref(), grant)
        {
            return Err(denied(caller));
        }
        if let Some(required) = &self.external
            && grant.is_some_and(|grant| !required.grants(grant))
        {
            return Err(denied("external"));
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(any(
    feature = "plugin",
    feature = "plugin-guest",
    feature = "plugin-sidecar"
))]
fn grant_allowed(
    required: Option<&harmos_contract::inventory::contracts::Required>,
    grant: Option<&harmos_contract::plugin::Grant>,
) -> bool {
    match grant {
        None => true,
        Some(grant) => required.is_some_and(|required| required.grants(grant)),
    }
}

async fn settle(entry: &Entry) -> Result<()> {
    let pending = entry
        .pending
        .lock()
        .expect("operation settlement lock is healthy")
        .clone();
    for settled in pending.into_values() {
        settled
            .settle()
            .await
            .map_err(|source| Error::OperationSettlement { source })?;
    }
    Ok(())
}

/* === Tests === */
