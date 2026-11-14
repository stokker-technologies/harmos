/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod function;
mod native;

#[cfg(all(feature = "grpc", feature = "streaming"))]
pub mod generated;

/* === Re-Exports === */

pub use function::{FunctionContext, FunctionHandle, FunctionInvocation};
pub use native::{NativeHost, NativeOperation};
#[cfg(feature = "streaming")]
pub use native::{NativeOutput, native_stream};

/* === Imports === */

// Standard Library Imports
use std::any::Any;
use std::future::Future;
use std::pin::Pin;

// Third Party Imports

// Local Imports
use super::{Carrier, Contract, ExecutionContext, OperationDefinition, RequestShape, Resource, ResourceSet, ServingError, WireDeclaration};
use crate::execution::services::Service;
use crate::execution::{Message, ServicePolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type ServiceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServingError>> + Send + 'a>>;
type InitializeService = fn(ExecutionContext) -> ServiceFuture<'static, Box<dyn ServiceInstance>>;
type PrepareNative = std::sync::Arc<
    dyn Fn(&OperationDefinition, ResourceSet, Box<dyn Any + Send>, Box<dyn Any + Send>) -> Result<Box<dyn NativeInstance>, ServingError>
        + Send
        + Sync,
>;
type PrepareEncoded = std::sync::Arc<
    dyn Fn(&OperationDefinition, ResourceSet, &[u8], bool, Option<super::WireStreamLimits>) -> Result<PreparedNative, ServingError>
        + Send
        + Sync,
>;

type OperationDependencies = std::sync::Arc<dyn Fn() -> Result<Vec<super::calls::OperationDependency>, ServingError> + Send + Sync>;
type DeclaredOperation = std::sync::Arc<dyn Fn() -> OperationDefinition + Send + Sync>;
type NamedOperation = std::sync::Arc<dyn Fn(&str) -> Result<OperationDefinition, ServingError> + Send + Sync>;
type GrantMapping =
    std::sync::Arc<dyn Fn(&str, Option<harmos_operations::plugin::Grant>) -> Option<harmos_operations::plugin::Grant> + Send + Sync>;
type RequestGrant = std::sync::Arc<
    dyn Fn(&(dyn Any + Send)) -> Result<Option<harmos_operations::plugin::Grant>, crate::execution::MessageError> + Send + Sync,
>;
type ContractProjection =
    std::sync::Arc<dyn Fn(&harmos_domain::config::SchemaSettings) -> Result<Contract, super::ContractError> + Send + Sync>;

/* === Structs === */

#[derive(Clone, Copy)]
pub struct CatalogDescription {
    pub name: &'static str,
    pub declarations: &'static [crate::execution::catalog::Node<WireDeclaration>],
    pub callees: &'static [crate::execution::catalog::Node<&'static [WireDeclaration]>],
    factory: Option<fn() -> Catalog>,
}

#[derive(Clone, Default)]
pub struct Catalog {
    pub name: &'static str,
    pub operations: Vec<OperationFactory>,
    pub services: Vec<ServiceFactory>,
}

pub struct PreparedNative {
    #[cfg(feature = "streaming")]
    pub streams: Option<super::WireStreams>,

    pub instance: Box<dyn NativeInstance>,
    pub grant: Option<harmos_operations::plugin::Grant>,
}

#[derive(Clone)]
pub struct OperationFactory {
    pub definition: OperationDefinition,
    pub operations: OperationDependencies,
    declared: DeclaredOperation,
    named: NamedOperation,
    registered_id: Option<crate::execution::Identifier>,
    grant: GrantMapping,
    request_grant: RequestGrant,
    pub(super) builtin: bool,
    resources: &'static [Resource],
    contract: ContractProjection,
    native: PrepareNative,
    endpoints_valid: fn() -> bool,
    native_types: fn() -> (std::any::TypeId, std::any::TypeId, std::any::TypeId),
    encoded: PrepareEncoded,

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    grpc_memory: fn(&[u8]) -> Result<usize, crate::execution::MessageError>,

    decoding_memory: fn(&[u8]) -> Result<usize, crate::execution::MessageError>,
}

#[derive(Clone)]
pub struct ServiceFactory {
    pub local_id: String,
    pub policy: ServicePolicy,
    initialize: InitializeService,
}

struct InitializedService<S: Service> {
    service: S,
}

struct CarrierOperation<O: Carrier> {
    dependencies: ResourceSet,
    operation: Option<O>,
    request: <O::Declaration as super::RequestShape>::Request,
    endpoints: O::StreamEndpoints,
    stopped: bool,
    encoded: bool,

    #[cfg(feature = "grpc")]
    protobuf: bool,

    #[cfg(feature = "streaming")]
    streams: Option<super::WireStreams>,

    #[cfg(feature = "streaming")]
    epoch: u64,
}

/* === Enums === */

/* === Traits === */

pub trait ServiceInstance: Send {
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ()>;
    fn stop(&mut self) -> ServiceFuture<'_, ()>;
    fn cleanup(&mut self) -> ServiceFuture<'_, ()>;
}

pub trait NativeInstance: Send {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::execution::operations::FailureKind;
    fn initialize(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ()>;
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, super::response::ErasedResponse>;
    fn stop(&mut self) -> ServiceFuture<'_, ()>;
    fn cleanup(&mut self) -> ServiceFuture<'_, ()>;
    #[cfg(feature = "streaming")]
    fn run_stream(&mut self, context: ExecutionContext, epoch: u64) -> ServiceFuture<'_, super::response::ErasedResponse> {
        let _ = (context, epoch);
        Box::pin(async { Err(ServingError::StreamUnsupported) })
    }
    fn transferred(&self) -> bool;

    #[cfg(feature = "streaming")]
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>>;
}

/* === Implementations === */

impl CatalogDescription {
    pub const fn new(
        name: &'static str, declarations: &'static [crate::execution::catalog::Node<WireDeclaration>],
        callees: &'static [crate::execution::catalog::Node<&'static [WireDeclaration]>], factory: Option<fn() -> Catalog>,
    ) -> Self {
        Self { name, declarations, callees, factory }
    }

    pub fn definitions(&self) -> Result<Vec<OperationDefinition>, crate::operations::Error> {
        if let Some(factory) = self.factory {
            let batch = factory();
            let mut names = std::collections::BTreeSet::new();
            for operation in &batch.operations {
                if !names.insert(operation.definition.id.clone()) {
                    return Err(crate::operations::Error::DuplicateOperation { id: operation.definition.id.to_string() });
                }
            }
            return Ok(batch.operations.into_iter().map(|operation| operation.definition).collect());
        }
        let mut entries = Vec::new();
        let result: Result<(), std::convert::Infallible> = crate::execution::catalog::try_for_each(self.declarations, |declaration| {
            entries.push(crate::execution::catalog::Node::Leaf(declaration.operation));
            Ok(())
        });
        match result {
            Ok(()) => {}
            Err(error) => match error {},
        }
        super::Declarations::from_tree(&entries)
    }

    #[doc(hidden)]
    pub fn identity(&self) -> &'static str {
        self.name
    }

    #[doc(hidden)]
    pub fn build(&self) -> Result<Catalog, ServingError> {
        let factory = self.factory.ok_or_else(|| ServingError::Unknown { operation: self.name.into() })?;
        Ok(factory())
    }
}

impl Catalog {
    pub fn named(mut self, name: &'static str) -> Self {
        self.name = name;
        self
    }
    pub fn build(self) -> Result<Self, ServingError> {
        let mut names = std::collections::BTreeSet::new();
        for operation in &self.operations {
            operation.validate_definition()?;
            if !names.insert(operation.definition.id.clone()) {
                return Err(ServingError::Declaration(crate::operations::Error::DuplicateOperation {
                    id: operation.definition.id.to_string(),
                }));
            }
        }
        Ok(self)
    }
    pub fn contracts(&self) -> Result<Vec<Contract>, super::ContractError> {
        self.operations.iter().map(OperationFactory::contract).collect()
    }
    pub fn definitions(&self) -> Result<Vec<OperationDefinition>, crate::operations::Error> {
        Ok(self.operations.iter().map(|operation| operation.definition.clone()).collect())
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn entry(mut self, operation: OperationFactory) -> Self {
        self.operations.push(operation);
        self
    }

    pub fn entry_named(self, operation: OperationFactory, id: &str) -> Result<Self, ServingError> {
        Ok(self.entry(operation.with_id(id)?))
    }

    pub fn extend(mut self, catalog: Catalog) -> Result<Self, ServingError> {
        let batch = catalog.build()?;
        self.operations.extend(batch.operations);
        self.services.extend(batch.services);
        Ok(self)
    }

    #[doc(hidden)]
    pub fn carrier<D, O>(mut self, specification: super::OperationSpec<D>) -> Result<Self, ServingError>
    where
        D: super::RequestShape,
        O: Carrier<Declaration = D>,
    {
        let definition = specification.declared().map_err(ServingError::Declaration)?;
        let declared = definition.clone();
        let named = specification;
        let original_id = specification.declaration.operation.id;
        let grant = specification.grant;
        let contract = specification.declaration;
        let callees = specification.callees;
        self.operations.push(OperationFactory {
            resources: &[],
            operations: std::sync::Arc::new(move || {
                callees
                    .iter()
                    .map(|declaration| {
                        Ok(super::calls::OperationDependency {
                            id: declaration.operation.id.to_owned(),
                            fingerprint: (declaration.operation.fingerprint)(declaration.operation.id)
                                .map_err(ServingError::Declaration)?,
                        })
                    })
                    .collect()
            }),
            definition,
            declared: std::sync::Arc::new(move || declared.clone()),
            named: std::sync::Arc::new(move |id| named.named(id).map_err(ServingError::Declaration)),
            registered_id: None,
            grant: std::sync::Arc::new(move |id, grant| registered_grant(original_id, id, grant)),
            request_grant: std::sync::Arc::new(move |request| Ok(request.downcast_ref::<D::Request>().and_then(grant))),
            builtin: matches!(original_id, "harmos_observe" | "harmos.initialize" | "harmos.shutdown"),
            contract: std::sync::Arc::new(move |settings| {
                if original_id == "harmos_observe" {
                    return crate::execution::observers::contract();
                }
                contract.contract_with_settings(settings)
            }),
            native_types: || {
                (
                    std::any::TypeId::of::<<O::Declaration as super::RequestShape>::Request>(),
                    std::any::TypeId::of::<<O::Declaration as super::RequestShape>::Response>(),
                    std::any::TypeId::of::<O::StreamEndpoints>(),
                )
            },
            endpoints_valid: || {
                use super::EndpointSet;
                O::StreamEndpoints::item_types() == super::item_types::<D>()
            },
            native: std::sync::Arc::new(|_definition, dependencies, request, endpoints| {
                use super::EndpointSet;
                if O::StreamEndpoints::item_types() != super::item_types::<D>() {
                    return Err(ServingError::Catalog);
                }
                let request = request.downcast::<<O::Declaration as super::RequestShape>::Request>().map_err(|_| ServingError::Catalog)?;
                let endpoints = endpoints.downcast::<O::StreamEndpoints>().map_err(|_| ServingError::Catalog)?;
                Ok(Box::new(CarrierOperation::<O> {
                    dependencies,
                    operation: None,
                    request: *request,
                    endpoints: *endpoints,
                    stopped: false,
                    encoded: false,
                    #[cfg(feature = "grpc")]
                    protobuf: false,
                    #[cfg(feature = "streaming")]
                    streams: None,
                    #[cfg(feature = "streaming")]
                    epoch: 0,
                }))
            }),
            encoded: std::sync::Arc::new(move |_definition, dependencies, payload, protobuf, limits| {
                use super::EndpointSet;
                #[cfg(not(feature = "streaming"))]
                let _ = limits;
                #[cfg(feature = "streaming")]
                let (endpoints, streams) = match O::StreamEndpoints::empty() {
                    Some(endpoints) => (endpoints, None),
                    None => {
                        let (endpoints, streams) = O::StreamEndpoints::bind_wire(limits.ok_or(ServingError::StreamLimits)?)?;
                        (endpoints, Some(streams))
                    }
                };
                #[cfg(not(feature = "streaming"))]
                let endpoints = O::StreamEndpoints::empty().ok_or(ServingError::StreamUnsupported)?;
                #[cfg(not(feature = "grpc"))]
                if protobuf {
                    return Err(ServingError::StreamUnsupported);
                }
                #[cfg(feature = "grpc")]
                let codec = protobuf.then(super::serving::protobuf::Codec::request::<D>);
                #[cfg(feature = "grpc")]
                let request = if let Some(codec) = codec {
                    let mut request = None::<D::Request>;
                    (codec.decode)(payload, &mut request).map_err(ServingError::Codec)?;
                    request.ok_or(ServingError::Catalog)?
                } else {
                    <D::Request as crate::execution::Message>::decode(payload).map_err(ServingError::Codec)?
                };
                #[cfg(not(feature = "grpc"))]
                let request = <D::Request as crate::execution::Message>::decode(payload).map_err(ServingError::Codec)?;
                #[cfg(all(feature = "grpc", feature = "streaming"))]
                let streams = match (streams, codec) {
                    (Some(streams), Some(codec)) => Some(streams.protobuf(codec)?),
                    (streams, _) => streams,
                };
                let grant = grant(&request);
                let instance = CarrierOperation::<O> {
                    dependencies,
                    operation: None,
                    request,
                    endpoints,
                    stopped: false,
                    encoded: true,
                    #[cfg(feature = "grpc")]
                    protobuf,
                    #[cfg(feature = "streaming")]
                    streams: streams.clone(),
                    #[cfg(feature = "streaming")]
                    epoch: 0,
                };
                Ok(PreparedNative {
                    instance: Box::new(instance),
                    grant,
                    #[cfg(feature = "streaming")]
                    streams,
                })
            }),
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            grpc_memory: |payload| {
                crate::execution::grpc::prepare::<<O::Declaration as super::RequestShape>::Request>(
                    crate::execution::grpc::DecodeInput::new(payload),
                )
                .map(|prepared| prepared.memory())
            },
            decoding_memory: <D::Request as crate::execution::Message>::decoding_memory,
        });
        Ok(self)
    }

    pub fn service<S: Service>(mut self, local_id: &str, policy: ServicePolicy) -> Self {
        self.services.push(ServiceFactory {
            local_id: local_id.to_owned(),
            policy,
            initialize: |context| {
                Box::pin(async move {
                    let service = S::initialize(context).await.map_err(service_error)?;
                    Ok(Box::new(InitializedService { service }) as Box<dyn ServiceInstance>)
                })
            },
        });
        self
    }
}

impl OperationFactory {
    #[doc(hidden)]
    pub fn prepare_carrier<C: Carrier>(
        dependencies: ResourceSet, request: <C::Declaration as RequestShape>::Request, endpoints: C::StreamEndpoints,
    ) -> Box<dyn NativeInstance> {
        Box::new(CarrierOperation::<C> {
            dependencies,
            operation: None,
            request,
            endpoints,
            stopped: false,
            encoded: false,
            #[cfg(feature = "grpc")]
            protobuf: false,
            #[cfg(feature = "streaming")]
            streams: None,
            #[cfg(feature = "streaming")]
            epoch: 0,
        })
    }

    /// Share Execution With an Independently Declared Face Contract.
    pub fn with_contract(mut self, contract: Contract) -> Self {
        self.definition = contract.operation().clone();
        self.declared = std::sync::Arc::new({
            let definition = self.definition.clone();
            move || definition.clone()
        });
        self.contract = std::sync::Arc::new(move |_| Ok(contract.clone()));
        self
    }

    pub fn with_id(mut self, id: &str) -> Result<Self, ServingError> {
        self.definition = (self.named)(id)?;
        self.registered_id = Some(self.definition.id.clone());
        Ok(self)
    }

    pub fn request_grant(
        &self, request: &(dyn Any + Send),
    ) -> Result<Option<harmos_operations::plugin::Grant>, crate::execution::MessageError> {
        (self.request_grant)(request)
    }
    pub fn accepts_target(&self, target: &super::Target, endpoints: std::any::TypeId) -> bool {
        (self.native_types)() == ((target.request)(), (target.response)(), endpoints)
    }

    pub fn accepts_native<D: RequestShape, P: 'static>(&self) -> bool {
        (self.native_types)() == (std::any::TypeId::of::<D::Request>(), std::any::TypeId::of::<D::Response>(), std::any::TypeId::of::<P>())
    }

    pub fn accepts_native_request<D: RequestShape>(&self) -> bool {
        let (request, response, _) = (self.native_types)();
        (request, response) == (std::any::TypeId::of::<D::Request>(), std::any::TypeId::of::<D::Response>())
    }

    // Every Declared Resource Comes From The Snapshot Or The Carrier's Execution Context.
    pub fn verify_resources(&self, resources: &ResourceSet, provided: &[std::any::TypeId]) -> Result<(), ServingError> {
        let missing = self.resources.iter().find(|resource| !resources.contains(resource.id()) && !provided.contains(&resource.id()));
        match missing {
            Some(resource) => Err(ServingError::Resource {
                operation: self.definition.id.to_string(),
                resource: resource.name(),
            }),
            None => Ok(()),
        }
    }

    pub fn validate(&self) -> Result<(), ServingError> {
        self.validate_definition()?;
        let declared = match &self.registered_id {
            Some(id) => (self.named)(id.as_str())?,
            None => (self.declared)(),
        };
        if self.definition != declared {
            return Err(ServingError::Catalog);
        }
        if !(self.endpoints_valid)() {
            return Err(ServingError::EndpointTypes);
        }
        Ok(())
    }

    fn validate_definition(&self) -> Result<(), ServingError> {
        let protobuf = self.contract().is_ok_and(|contract| contract.protobuf().is_some());
        if !self.builtin && !protobuf {
            self.definition.validate().map_err(ServingError::from)?;
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn missing_resources(&self, resources: &ResourceSet, provided: &[std::any::TypeId]) -> Vec<super::StartupError> {
        self.resources
            .iter()
            .filter(|resource| !resources.contains(resource.id()) && !provided.contains(&resource.id()))
            .map(|resource| super::StartupError::MissingResource {
                operation: self.definition.id.to_string(),
                resource: resource.name(),
            })
            .collect()
    }

    pub fn decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::execution::MessageError> {
        (self.decoding_memory)(payload)
    }

    #[doc(hidden)]
    pub fn prepare_envelope(
        &self, resources: ResourceSet, payload: &[u8], protobuf: bool, limits: Option<super::WireStreamLimits>,
    ) -> Result<PreparedNative, ServingError> {
        let mut prepared = (self.encoded)(&self.definition, resources, payload, protobuf, limits)?;
        prepared.grant = self.grant(prepared.grant);
        Ok(prepared)
    }

    pub fn prepare_encoded(&self, resources: ResourceSet, payload: &[u8]) -> Result<PreparedNative, ServingError> {
        let mut prepared = (self.encoded)(&self.definition, resources, payload, false, None)?;
        prepared.grant = self.grant(prepared.grant);
        Ok(prepared)
    }

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    pub fn prepare_grpc(
        &self, resources: ResourceSet, payload: &[u8], limits: super::WireStreamLimits,
    ) -> Result<PreparedNative, ServingError> {
        let mut prepared = (self.encoded)(&self.definition, resources, payload, true, Some(limits))?;
        prepared.grant = self.grant(prepared.grant);
        Ok(prepared)
    }

    pub fn grant(&self, grant: Option<harmos_operations::plugin::Grant>) -> Option<harmos_operations::plugin::Grant> {
        (self.grant)(self.definition.id.as_str(), grant)
    }
    #[cfg(all(feature = "grpc", feature = "streaming"))]
    pub fn grpc_decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::execution::MessageError> {
        (self.grpc_memory)(payload)
    }

    pub fn prepare_native(
        &self, resources: ResourceSet, request: Box<dyn Any + Send>, endpoints: Box<dyn Any + Send>,
    ) -> Result<Box<dyn NativeInstance>, ServingError> {
        (self.native)(&self.definition, resources, request, endpoints)
    }

    pub fn contract(&self) -> Result<Contract, super::ContractError> {
        self.contract_with_settings(&harmos_domain::config::SchemaSettings::default())
    }

    pub fn contract_with_settings(&self, settings: &harmos_domain::config::SchemaSettings) -> Result<Contract, super::ContractError> {
        let contract = (self.contract)(settings)?;
        match &self.registered_id {
            Some(id) => contract.with_id(id.to_string()),
            None => Ok(contract),
        }
    }
}

impl<O: Carrier> NativeInstance for CarrierOperation<O> {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::execution::operations::FailureKind {
        error.downcast_ref::<O::Error>().map_or(crate::execution::operations::FailureKind::Permanent, O::classify)
    }

    fn initialize(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async move {
            if self.operation.is_none() {
                let context = context.with_resources(self.dependencies.clone());
                self.operation = Some(O::bind(context).await.map_err(operation_error::<O>)?);
            }
            Ok(())
        })
    }

    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, super::response::ErasedResponse> {
        Box::pin(async move {
            let operation = self.operation.as_mut().ok_or(ServingError::Uninitialized)?;
            let context = context.with_resources(self.dependencies.clone());
            #[cfg(feature = "streaming")]
            let attempt = if let Some(streams) = &self.streams {
                self.epoch = self.epoch.checked_add(1).ok_or(ServingError::StreamPhase)?;
                Some(streams.start(self.epoch)?)
            } else {
                None
            };
            let response = operation.attempt(context, &mut self.request, &mut self.endpoints).await.map_err(operation_error::<O>)?;
            #[cfg(feature = "streaming")]
            if let Some(attempt) = attempt {
                attempt.end();
            }
            #[cfg(feature = "grpc")]
            if self.protobuf {
                let mut bytes = Vec::new();
                crate::execution::grpc::encode(&*response, &mut bytes).map_err(ServingError::Codec)?;
                return Ok(super::Response::new(bytes).into_erased());
            }
            if self.encoded {
                return Ok(super::Response::new(response.encode()).into_erased());
            }
            Ok(response.into_erased())
        })
    }

    #[cfg(feature = "streaming")]
    fn run_stream(&mut self, context: ExecutionContext, epoch: u64) -> ServiceFuture<'_, super::response::ErasedResponse> {
        if self.streams.is_none() || self.epoch.checked_add(1) != Some(epoch) {
            return Box::pin(async { Err(ServingError::StreamPhase) });
        }
        self.run(context)
    }

    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async move {
            if self.stopped {
                return Ok(());
            }
            if let Some(operation) = self.operation.as_mut() {
                operation.stop().await.map_err(operation_error::<O>)?;
            }
            self.stopped = true;
            Ok(())
        })
    }

    fn cleanup(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async move {
            if let Some(operation) = self.operation.as_mut() {
                operation.cleanup().await.map_err(operation_error::<O>)?;
            }
            self.operation = None;
            Ok(())
        })
    }

    #[cfg(feature = "streaming")]
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        super::EndpointSet::progress(&self.endpoints)
    }

    fn transferred(&self) -> bool {
        use super::EndpointSet;
        self.endpoints.transferred() || self.operation.as_ref().is_some_and(Carrier::transferred)
    }
}

impl ServiceFactory {
    pub fn initialize(&self, context: ExecutionContext) -> ServiceFuture<'static, Box<dyn ServiceInstance>> {
        (self.initialize)(context)
    }
}

impl<S: Service> ServiceInstance for InitializedService<S> {
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async move { self.service.run(context).await.map_err(service_error) })
    }
    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async move { self.service.stop().await.map_err(service_error) })
    }
    fn cleanup(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async move { self.service.cleanup().await.map_err(service_error) })
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(crate) fn registered_grant(
    original: &str, id: &str, grant: Option<harmos_operations::plugin::Grant>,
) -> Option<harmos_operations::plugin::Grant> {
    grant.map(|mut grant| {
        if grant.operation() == original && id != original {
            grant.operation = id.to_owned().into();
        }
        grant
    })
}

fn service_error(error: impl std::error::Error + 'static) -> ServingError {
    ServingError::Handler(super::StartupError::failure(&error, crate::execution::operations::FailureKind::Permanent))
}

fn operation_error<O: Carrier>(error: O::Error) -> ServingError {
    ServingError::Native(std::sync::Arc::new(error))
}

/* === Tests === */
