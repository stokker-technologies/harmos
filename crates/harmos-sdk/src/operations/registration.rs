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
use std::any::Any;
use std::future::Future;
use std::pin::Pin;

// Third Party Imports

// Local Imports
use super::{
    Carrier, Contract, Executable, ExecutionContext, OperationDeclaration, OperationDefinition, Resource, ResourceSet, ServingError,
    WireDeclaration, WireTable,
};
use crate::services::Service;
use crate::{Message, ServicePolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type ServiceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServingError>> + Send + 'a>>;
type Install = fn(&mut WireTable, ResourceSet, Contract) -> Result<(), ServingError>;
type InitializeService = fn(ExecutionContext) -> ServiceFuture<'static, Box<dyn ServiceInstance>>;
type PrepareNative = fn(ResourceSet, Box<dyn Any + Send>, Box<dyn Any + Send>) -> Result<Box<dyn NativeInstance>, ServingError>;
type PrepareEncoded = fn(ResourceSet, &[u8]) -> Result<PreparedNative, ServingError>;

/* === Structs === */

#[derive(Clone, Copy)]
pub struct Catalog {
    pub name: &'static str,
    pub declarations: &'static [crate::catalog::Node<WireDeclaration>],
    pub callees: &'static [crate::catalog::Node<&'static [WireDeclaration]>],
    identity: fn() -> std::any::TypeId,
    factory: fn() -> RegistrationBatch,
}

#[derive(Clone, Default)]
pub struct RegistrationBatch {
    pub operations: Vec<OperationFactory>,
    pub services: Vec<ServiceFactory>,
}

pub struct PreparedNative {
    #[cfg(feature = "streaming")]
    pub streams: Option<super::WireStreams>,

    pub instance: Box<dyn NativeInstance>,
    pub grant: Option<crate::plugin::Grant>,
    pub key: Option<super::CacheKey>,
}

#[derive(Clone)]
pub struct OperationFactory {
    pub definition: OperationDefinition,
    pub operations: &'static [WireDeclaration],
    declared: fn() -> OperationDefinition,
    builtin: bool,
    resources: &'static [Resource],
    contract: fn(&harmos_domain::config::SchemaSettings) -> Result<Contract, super::ContractError>,
    install: Install,
    native: PrepareNative,
    endpoints_valid: fn() -> bool,
    native_types: fn() -> (std::any::TypeId, std::any::TypeId, std::any::TypeId),
    encoded: PrepareEncoded,

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    grpc: fn(ResourceSet, &[u8], super::WireStreamLimits) -> Result<PreparedNative, ServingError>,

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    grpc_memory: fn(&[u8]) -> Result<usize, crate::MessageError>,

    decoding_memory: fn(&[u8]) -> Result<usize, crate::MessageError>,
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

struct NativeOperation<O: Carrier> {
    dependencies: ResourceSet,
    operation: Option<O>,
    request: <O::Declaration as OperationDeclaration>::Request,
    endpoints: O::StreamEndpoints,
    stopped: bool,
    encoded: bool,

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    protobuf: bool,

    #[cfg(feature = "streaming")]
    streams: Option<super::WireStreams>,

    #[cfg(feature = "streaming")]
    epoch: u64,
}

/* === Enums === */

/* === Traits === */

pub trait OperationCatalog: 'static {
    const CATALOG: Catalog;

    #[doc(hidden)]
    fn batch() -> RegistrationBatch;
}

pub trait ServiceInstance: Send {
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ()>;
    fn stop(&mut self) -> ServiceFuture<'_, ()>;
    fn cleanup(&mut self) -> ServiceFuture<'_, ()>;
}

pub trait NativeInstance: Send {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind;
    fn initialize(&mut self, context: ExecutionContext) -> ServiceFuture<'_, ()>;
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, super::response::ErasedResponse>;
    fn stop(&mut self) -> ServiceFuture<'_, ()>;
    fn cleanup(&mut self) -> ServiceFuture<'_, ()>;
    fn transferred(&self) -> bool;
}

/* === Implementations === */

impl Catalog {
    pub const fn of<C: OperationCatalog>(
        name: &'static str, declarations: &'static [crate::catalog::Node<WireDeclaration>],
        callees: &'static [crate::catalog::Node<&'static [WireDeclaration]>],
    ) -> Self {
        Self {
            name,
            declarations,
            callees,
            identity: std::any::TypeId::of::<C>,
            factory: C::batch,
        }
    }

    #[doc(hidden)]
    pub fn identity(&self) -> std::any::TypeId {
        (self.identity)()
    }

    #[doc(hidden)]
    pub fn batch(&self) -> RegistrationBatch {
        (self.factory)()
    }
}

impl RegistrationBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operation<O: Executable>(self) -> Self {
        self.with::<O, O::Carrier>(O::RESOURCES)
    }

    #[doc(hidden)]
    pub fn carrier<D, O>(self) -> Self
    where
        D: OperationDeclaration,
        O: Carrier<Declaration = D>,
    {
        self.with::<D, O>(&[])
    }

    fn with<D, O>(mut self, resources: &'static [Resource]) -> Self
    where
        D: OperationDeclaration,
        O: Carrier<Declaration = D>,
    {
        self.operations.push(OperationFactory {
            resources,
            operations: D::OPERATIONS,
            definition: D::definition(),
            declared: D::definition,
            builtin: std::any::TypeId::of::<D>() == std::any::TypeId::of::<crate::observers::Control>(),
            contract: |settings| {
                if std::any::TypeId::of::<D>() == std::any::TypeId::of::<crate::observers::Control>() {
                    return crate::observers::contract();
                }
                WireDeclaration::of::<D>().contract_with_settings(settings)
            },
            install: WireTable::install::<D, O>,
            native_types: || {
                (
                    std::any::TypeId::of::<<O::Declaration as OperationDeclaration>::Request>(),
                    std::any::TypeId::of::<<O::Declaration as OperationDeclaration>::Response>(),
                    std::any::TypeId::of::<O::StreamEndpoints>(),
                )
            },
            endpoints_valid: || {
                use super::EndpointSet;
                O::StreamEndpoints::item_types() == super::item_types::<D>()
            },
            native: |dependencies, request, endpoints| {
                use super::EndpointSet;
                if O::StreamEndpoints::item_types() != super::item_types::<D>() {
                    return Err(ServingError::Catalog);
                }
                let request = request.downcast::<<O::Declaration as OperationDeclaration>::Request>().map_err(|_| ServingError::Catalog)?;
                let endpoints = endpoints.downcast::<O::StreamEndpoints>().map_err(|_| ServingError::Catalog)?;
                Ok(Box::new(NativeOperation::<O> {
                    dependencies,
                    operation: None,
                    request: *request,
                    endpoints: *endpoints,
                    stopped: false,
                    encoded: false,
                    #[cfg(all(feature = "grpc", feature = "streaming"))]
                    protobuf: false,
                    #[cfg(feature = "streaming")]
                    streams: None,
                    #[cfg(feature = "streaming")]
                    epoch: 0,
                }))
            },
            encoded: |dependencies, payload| {
                use super::EndpointSet;
                let endpoints = O::StreamEndpoints::empty().ok_or(ServingError::StreamUnsupported)?;
                let request =
                    <<O::Declaration as OperationDeclaration>::Request as crate::Message>::decode(payload).map_err(ServingError::Codec)?;
                let grant = D::grant(&request);
                let key = D::key(&request);
                let instance = NativeOperation::<O> {
                    dependencies,
                    operation: None,
                    request,
                    endpoints,
                    stopped: false,
                    encoded: true,
                    #[cfg(all(feature = "grpc", feature = "streaming"))]
                    protobuf: false,
                    #[cfg(feature = "streaming")]
                    streams: None,
                    #[cfg(feature = "streaming")]
                    epoch: 0,
                };
                Ok(PreparedNative {
                    instance: Box::new(instance),
                    grant,
                    key,
                    #[cfg(feature = "streaming")]
                    streams: None,
                })
            },
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            grpc: |dependencies, payload, limits| {
                use super::EndpointSet;
                let request =
                    crate::grpc::decode::<<O::Declaration as OperationDeclaration>::Request>(crate::grpc::DecodeInput::new(payload))
                        .map_err(ServingError::Codec)?;
                let (endpoints, streams) = match O::StreamEndpoints::empty() {
                    Some(endpoints) => (endpoints, None),
                    None => {
                        let (endpoints, streams) = O::StreamEndpoints::bind_wire(limits)?;
                        (endpoints, Some(streams.protobuf(super::serving::protobuf::Codec::of::<D>())?))
                    }
                };
                let grant = D::grant(&request);
                let key = D::key(&request);
                let instance = NativeOperation::<O> {
                    dependencies,
                    operation: None,
                    request,
                    endpoints,
                    stopped: false,
                    encoded: true,
                    protobuf: true,
                    streams: streams.clone(),
                    epoch: 0,
                };
                Ok(PreparedNative {
                    instance: Box::new(instance),
                    grant,
                    key,
                    streams,
                })
            },
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            grpc_memory: |payload| {
                crate::grpc::prepare::<<O::Declaration as OperationDeclaration>::Request>(crate::grpc::DecodeInput::new(payload))
                    .map(|prepared| prepared.memory())
            },
            decoding_memory: <D::Request as crate::Message>::decoding_memory,
        });
        self
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
    pub fn accepts_target(&self, target: &super::Target) -> bool {
        (self.native_types)() == ((target.request)(), (target.response)(), std::any::TypeId::of::<()>())
    }

    pub fn accepts_native<D: OperationDeclaration, P: 'static>(&self) -> bool {
        (self.native_types)() == (std::any::TypeId::of::<D::Request>(), std::any::TypeId::of::<D::Response>(), std::any::TypeId::of::<P>())
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
        if !self.builtin {
            self.definition.validate().map_err(ServingError::from)?;
        }
        if self.definition != (self.declared)() {
            return Err(ServingError::Catalog);
        }
        if !(self.endpoints_valid)() {
            return Err(ServingError::EndpointTypes);
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

    pub fn decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::MessageError> {
        (self.decoding_memory)(payload)
    }

    pub fn prepare_encoded(&self, resources: ResourceSet, payload: &[u8]) -> Result<PreparedNative, ServingError> {
        (self.encoded)(resources, payload)
    }

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    pub fn prepare_grpc(
        &self, resources: ResourceSet, payload: &[u8], limits: super::WireStreamLimits,
    ) -> Result<PreparedNative, ServingError> {
        (self.grpc)(resources, payload, limits)
    }
    #[cfg(all(feature = "grpc", feature = "streaming"))]
    pub fn grpc_decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::MessageError> {
        (self.grpc_memory)(payload)
    }

    pub fn prepare_native(
        &self, resources: ResourceSet, request: Box<dyn Any + Send>, endpoints: Box<dyn Any + Send>,
    ) -> Result<Box<dyn NativeInstance>, ServingError> {
        (self.native)(resources, request, endpoints)
    }

    pub fn contract(&self) -> Result<Contract, super::ContractError> {
        self.contract_with_settings(&harmos_domain::config::SchemaSettings::default())
    }

    pub fn contract_with_settings(&self, settings: &harmos_domain::config::SchemaSettings) -> Result<Contract, super::ContractError> {
        (self.contract)(settings)
    }

    pub fn install(self, table: &mut WireTable) -> Result<(), ServingError> {
        self.validate()?;
        self.verify_resources(table.owner_resources(), table.provided())?;
        let contract = self.contract()?;
        let resources = table.owner_resources().clone();
        (self.install)(table, resources, contract)
    }
}

impl<O: Carrier> NativeInstance for NativeOperation<O> {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind {
        error.downcast_ref::<O::Error>().map_or(crate::operations::FailureKind::Permanent, O::classify)
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
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            if self.protobuf {
                let mut bytes = Vec::new();
                crate::grpc::encode(&*response, &mut bytes).map_err(ServingError::Codec)?;
                return Ok(super::Response::new(bytes).into_erased());
            }
            if self.encoded {
                return Ok(super::Response::new(response.encode()).into_erased());
            }
            Ok(response.into_erased())
        })
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

fn service_error(error: impl std::error::Error + 'static) -> ServingError {
    ServingError::Handler(crate::Failure::originate(crate::operations::FailureKind::Permanent, &error, crate::failure::Origin::Plugin))
}

fn operation_error<O: Carrier>(error: O::Error) -> ServingError {
    ServingError::Native(std::sync::Arc::new(error))
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct Worker(Arc<AtomicUsize>);

    struct ReadCatalog;

    impl OperationCatalog for ReadCatalog {
        const CATALOG: Catalog = Catalog::of::<Self>("ReadCatalog", &[], &[]);
        fn batch() -> RegistrationBatch {
            RegistrationBatch::new().operation::<Read>()
        }
    }

    #[crate::operation(harmos_path = crate, resources = [u64], id = "fixture.read", response = u64,
        policy = crate::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            retries: crate::RetryPolicy::Never,
        },
        key = |_: &Read| Some(super::super::CacheKey::new(vec![42]).unwrap()),
    )]
    struct Read;

    impl crate::Operation for Read {
        type Error = std::convert::Infallible;

        async fn run(self, ctx: crate::Context<'_, Self>) -> Result<crate::Response<u64>, Self::Error> {
            Ok(crate::Response::from_owner(ctx.resource::<u64>()))
        }
    }

    fn ready<T>(mut future: ServiceFuture<'_, T>) -> T {
        let mut task = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut task) {
            std::task::Poll::Ready(Ok(value)) => value,
            _ => panic!("synchronous fixture must complete successfully"),
        }
    }

    #[test]
    fn carrier_assignments_are_immutable() {
        use crate::operations::{AssignedOperation, OperationAssignments};
        let first = AssignedOperation {
            operation: format!("fixture.{}", Read::ID),
            fingerprint: Read::definition().fingerprint.0,
            generation: 1,
        };
        let assignments = OperationAssignments::default();
        let bytes = AssignedOperation::encode(std::slice::from_ref(&first)).unwrap();
        assignments.install(&bytes).unwrap();
        assignments.clone().install(&bytes).unwrap();
        assert_eq!(assignments.resolve::<Read>().unwrap(), &first);
        let mut replacement = first.clone();
        replacement.generation = 2;
        assert!(assignments.install(&AssignedOperation::encode(&[replacement]).unwrap()).is_err());
        assert_eq!(assignments.resolve::<Read>().unwrap().generation, 1);
        let mut wrong = first.clone();
        wrong.fingerprint[0] ^= 1;
        let incompatible = OperationAssignments::default();
        incompatible.install(&AssignedOperation::encode(&[wrong]).unwrap()).unwrap();
        assert!(incompatible.resolve::<Read>().is_err());
        let duplicate = AssignedOperation {
            operation: format!("other.{}", Read::ID),
            ..first.clone()
        };
        assert!(OperationAssignments::default().install(&AssignedOperation::encode(&[first, duplicate]).unwrap()).is_err());
    }

    #[test]
    fn native_response_retains_owner_across_retries_and_cleanup() {
        let value = Arc::new(42u64);
        let retained = Arc::downgrade(&value);
        let mut dependencies = ResourceSet::new();
        dependencies.insert(value).unwrap();
        let batch = RegistrationBatch::new().operation::<Read>();
        let factory = &batch.operations[0];
        assert!(factory.prepare_native(dependencies.clone(), Box::new(7u64), Box::new(())).is_err());
        let mut invocation = factory.prepare_native(dependencies, Box::new(Read), Box::new(())).unwrap();
        let context = ExecutionContext::new(ResourceSet::new(), super::super::CallContext::default());

        ready(invocation.initialize(context.clone()));
        let first = ready(invocation.run(context.clone())).restore::<u64>().unwrap();
        let second = ready(invocation.run(context)).restore::<u64>().unwrap();
        assert!(first.ptr_eq(&second));
        assert_eq!(*first, 42);
        ready(invocation.stop());
        ready(invocation.cleanup());

        drop(invocation);
        drop(batch);
        assert!(retained.upgrade().is_some());
        drop(first);
        drop(second);
        assert!(retained.upgrade().is_none());
    }

    #[test]
    fn encoded_factory_preserves_request_cache_key_and_response() {
        use crate::Message;

        let mut dependencies = ResourceSet::new();
        dependencies.insert(Arc::new(42u64)).unwrap();
        let batch = RegistrationBatch::new().operation::<Read>();
        let mut prepared = batch.operations[0].prepare_encoded(dependencies, &Read.encode()).unwrap();
        assert_eq!(prepared.key, Read::key(&Read));
        let context = ExecutionContext::new(ResourceSet::new(), super::super::CallContext::default());
        ready(prepared.instance.initialize(context.clone()));
        let response = ready(prepared.instance.run(context)).restore::<Vec<u8>>().unwrap();
        assert_eq!(u64::decode(&response).unwrap(), 42);
        ready(prepared.instance.stop());
        ready(prepared.instance.cleanup());
    }

    impl Service for Worker {
        type Error = crate::ResourceError;

        async fn initialize(context: ExecutionContext) -> Result<Self, Self::Error> {
            let counter = context.resource::<AtomicUsize>()?;
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Self(counter))
        }

        async fn run(&mut self, _: ExecutionContext) -> Result<(), Self::Error> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn factory_rejects_a_substituted_declaration() {
        let mut batch = RegistrationBatch::new().operation::<Read>();
        batch.operations[0].definition.id = crate::Identifier::new("different").unwrap();
        assert!(matches!(batch.operations[0].validate(), Err(ServingError::Catalog)));
    }

    #[test]
    fn wire_identity_fences_replacement_before_decoding() {
        use crate::operations::{ServingError, WireTable};
        use crate::{Message, QualifiedId};
        let batch = RegistrationBatch::new().operation::<Read>();
        let fingerprint = crate::OperationDefinition::of::<Read>().unwrap().fingerprint.0;
        let mut table = WireTable::new("fixture").unwrap();
        table.resource(42u64);
        table.capacity(std::num::NonZeroUsize::MIN);
        table.register(batch.clone()).unwrap();
        table.validate_binding().unwrap();
        assert!(matches!(table.prepare(1, "spoof.fixture.read", fingerprint, 1, Read.encode()), Err(ServingError::Unknown { .. })));
        assert!(matches!(table.prepare(1, "fixture.fixture.read", [0; 32], 1, vec![255]), Err(ServingError::Identity { .. })));
        assert!(matches!(table.prepare(1, "fixture.fixture.read", fingerprint, 0, vec![255]), Err(ServingError::Identity { .. })));
        table.prepare(1, "fixture.fixture.read", fingerprint, 7, Read.encode()).unwrap();
        ready(Box::pin(table.unregister(&QualifiedId::qualify("fixture", "fixture.read").unwrap())));
        table.register(batch).unwrap();
        assert!(matches!(table.prepare(2, "fixture.fixture.read", fingerprint, 7, vec![255]), Err(ServingError::Identity { .. })));
        table.prepare(2, "fixture.fixture.read", fingerprint, 8, Read.encode()).unwrap();
        assert!(matches!(table.prepare(3, "fixture.fixture.read", fingerprint, 9, vec![255]), Err(ServingError::Identity { .. })));
        ready(Box::pin(table.settle()));
    }

    #[test]
    fn service_factory_reconstructs_from_its_snapshot() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut dependencies = ResourceSet::new();
        dependencies.insert(counter.clone()).unwrap();
        let policy = ServicePolicy {
            stop_timeout: std::time::Duration::from_secs(1),
            restart: crate::RestartPolicy::Never,
        };
        let batch = RegistrationBatch::new().service::<Worker>("worker", policy);
        let factory = &batch.services[0];
        let context = ExecutionContext::new(dependencies, super::super::CallContext::default());
        let mut first = factory.initialize(context.clone());
        let mut second = factory.initialize(context);
        let mut task = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(matches!(first.as_mut().poll(&mut task), std::task::Poll::Ready(Ok(_))));
        assert!(matches!(second.as_mut().poll(&mut task), std::task::Poll::Ready(Ok(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn binding_errors_aggregate() {
        use crate::StartupError;
        let mut table = WireTable::new("fixture").unwrap();
        table.resource(7u32).resource(8u32);
        table.catalog(&ReadCatalog::CATALOG).catalog(&ReadCatalog::CATALOG);
        assert!(table.definitions().is_empty());

        let Err(ServingError::Startup(error)) = table.validate_binding() else {
            panic!("invalid binding must be refused");
        };
        let StartupError::Multiple { errors } = *error else {
            panic!("independent binding errors must be aggregated");
        };
        assert_eq!(errors.len(), 3);
        assert!(errors.iter().any(|error| matches!(error, StartupError::DuplicateResource { .. })));
        assert!(errors.iter().any(|error| matches!(error, StartupError::DuplicateCatalog { .. })));
        assert!(errors.iter().any(|error| matches!(error, StartupError::MissingResource { .. })));
        assert!(table.definitions().is_empty());
        assert_eq!(*table.context().resource::<u32>().unwrap(), 7);
    }

    #[test]
    fn frozen_dynamic_resources() {
        let mut table = WireTable::new("fixture").unwrap();
        let owner = Arc::new(41u32);
        table.resource::<u32>(owner.clone());
        table.validate_binding().unwrap();
        assert!(Arc::ptr_eq(&owner, &table.context().resource::<u32>().unwrap()));

        let error = table.register(ReadCatalog::batch()).unwrap_err();
        assert!(matches!(error, ServingError::Resource { .. }));
        table.resource(42u64);
        assert!(table.validate_binding().is_err());
        assert!(table.context().resource::<u64>().is_err());
        assert!(table.definitions().is_empty());
    }

    #[test]
    fn resource_order_independence() {
        let mut table = WireTable::new("fixture").unwrap();
        table.catalog(&ReadCatalog::CATALOG).resource(42u64);
        assert!(table.definitions().is_empty());
        table.validate_binding().unwrap();
        assert_eq!(table.definitions().len(), 1);
        assert_eq!(*table.context().resource::<u64>().unwrap(), 42);
    }
}
