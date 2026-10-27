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
    Contract, Dependencies, Executable, Handler, OperationContext, OperationDeclaration, OperationDefinition, Resource, ServingError,
    WireDeclaration, WireTable,
};
use crate::ServicePolicy;
use crate::services::Service;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type ServiceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ServingError>> + Send + 'a>>;
type Install = fn(&mut WireTable, Dependencies, Contract) -> Result<(), ServingError>;
type InitializeService = fn(OperationContext) -> ServiceFuture<'static, Box<dyn ServiceInstance>>;
type PrepareNative = fn(Dependencies, Box<dyn Any + Send>, Box<dyn Any + Send>) -> Result<Box<dyn NativeInstance>, ServingError>;
type PrepareEncoded = fn(Dependencies, &[u8]) -> Result<PreparedNative, ServingError>;

/* === Structs === */

#[derive(Clone)]
pub struct RegistrationBatch {
    pub dependencies: Dependencies,
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
    pub dependencies: Dependencies,
    contract: fn() -> Result<Contract, super::ContractError>,
    install: Install,
    native: PrepareNative,
    endpoints_valid: fn() -> bool,
    native_types: fn() -> (std::any::TypeId, std::any::TypeId, std::any::TypeId),
    encoded: PrepareEncoded,
    #[cfg(all(feature = "grpc", feature = "streaming"))]
    grpc: fn(Dependencies, &[u8], super::WireStreamLimits) -> Result<PreparedNative, ServingError>,
    #[cfg(all(feature = "grpc", feature = "streaming"))]
    grpc_memory: fn(&[u8]) -> Result<usize, crate::MessageError>,
    decoding_memory: fn(&[u8]) -> Result<usize, crate::MessageError>,
}

#[derive(Clone)]
pub struct ServiceFactory {
    pub local_id: String,
    pub policy: ServicePolicy,
    pub dependencies: Dependencies,
    initialize: InitializeService,
}

struct InitializedService<S: Service> {
    service: S,
    dependencies: Dependencies,
}

struct NativeOperation<O: Handler> {
    dependencies: Dependencies,
    operation: Option<O>,
    request: O::Request,
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

pub trait ServiceInstance: Send {
    fn run(&mut self, context: OperationContext) -> ServiceFuture<'_, ()>;
    fn stop(&mut self) -> ServiceFuture<'_, ()>;
    fn cleanup(&mut self) -> ServiceFuture<'_, ()>;
}

pub trait NativeInstance: Send {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind;
    fn initialize(&mut self, context: OperationContext) -> ServiceFuture<'_, ()>;
    fn run(&mut self, context: OperationContext) -> ServiceFuture<'_, super::response::ErasedResponse>;
    fn stop(&mut self) -> ServiceFuture<'_, ()>;
    fn cleanup(&mut self) -> ServiceFuture<'_, ()>;
    fn transferred(&self) -> bool;
}

/* === Implementations === */

impl RegistrationBatch {
    pub fn new(dependencies: Dependencies) -> Self {
        Self {
            dependencies,
            operations: Vec::new(),
            services: Vec::new(),
        }
    }

    pub fn operation<O: Executable>(self) -> Self {
        self.with::<O, O::Handler>(O::RESOURCES)
    }

    pub fn implementation<D, O>(self) -> Self
    where
        D: OperationDeclaration,
        O: Handler<Request = D::Request, Response = D::Response>,
    {
        self.with::<D, O>(&[])
    }

    fn with<D, O>(mut self, resources: &'static [Resource]) -> Self
    where
        D: OperationDeclaration,
        O: Handler<Request = D::Request, Response = D::Response>,
    {
        self.operations.push(OperationFactory {
            resources,
            operations: D::OPERATIONS,
            definition: D::definition(),
            declared: D::definition,
            builtin: std::any::TypeId::of::<D>() == std::any::TypeId::of::<crate::observers::Control>(),
            dependencies: self.dependencies.clone(),
            contract: || {
                if std::any::TypeId::of::<D>() == std::any::TypeId::of::<crate::observers::Control>() {
                    return crate::observers::contract();
                }
                WireDeclaration::of::<D>().contract()
            },
            install: WireTable::install::<D, O>,
            native_types: || {
                (std::any::TypeId::of::<O::Request>(), std::any::TypeId::of::<O::Response>(), std::any::TypeId::of::<O::StreamEndpoints>())
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
                let request = request.downcast::<O::Request>().map_err(|_| ServingError::Catalog)?;
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
                use crate::Message;
                let endpoints = O::StreamEndpoints::empty().ok_or(ServingError::StreamUnsupported)?;
                let request = O::Request::decode(payload).map_err(ServingError::Codec)?;
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
                let request = crate::grpc::decode::<O::Request>(crate::grpc::DecodeInput::new(payload)).map_err(ServingError::Codec)?;
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
                crate::grpc::prepare::<O::Request>(crate::grpc::DecodeInput::new(payload)).map(|prepared| prepared.memory())
            },
            decoding_memory: <D::Request as crate::Message>::decoding_memory,
        });
        self
    }

    pub fn service<S: Service>(mut self, local_id: &str, policy: ServicePolicy) -> Self {
        self.services.push(ServiceFactory {
            local_id: local_id.to_owned(),
            policy,
            dependencies: self.dependencies.clone(),
            initialize: |context| {
                Box::pin(async move {
                    let dependencies = context.dependencies().clone();
                    let service = S::initialize(context).await.map_err(service_error)?;
                    Ok(Box::new(InitializedService { service, dependencies }) as Box<dyn ServiceInstance>)
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
    pub fn verify_resources(&self, provided: &[std::any::TypeId]) -> Result<(), ServingError> {
        let missing =
            self.resources.iter().find(|resource| !self.dependencies.contains(resource.id()) && !provided.contains(&resource.id()));
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

    pub fn decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::MessageError> {
        (self.decoding_memory)(payload)
    }

    pub fn prepare_encoded(&self, payload: &[u8]) -> Result<PreparedNative, ServingError> {
        (self.encoded)(self.dependencies.clone(), payload)
    }

    #[cfg(all(feature = "grpc", feature = "streaming"))]
    pub fn prepare_grpc(&self, payload: &[u8], limits: super::WireStreamLimits) -> Result<PreparedNative, ServingError> {
        (self.grpc)(self.dependencies.clone(), payload, limits)
    }
    #[cfg(all(feature = "grpc", feature = "streaming"))]
    pub fn grpc_decoding_memory(&self, payload: &[u8]) -> Result<usize, crate::MessageError> {
        (self.grpc_memory)(payload)
    }

    pub fn prepare_native(
        &self, request: Box<dyn Any + Send>, endpoints: Box<dyn Any + Send>,
    ) -> Result<Box<dyn NativeInstance>, ServingError> {
        (self.native)(self.dependencies.clone(), request, endpoints)
    }

    pub fn contract(&self) -> Result<Contract, super::ContractError> {
        (self.contract)()
    }

    pub fn install(self, table: &mut WireTable) -> Result<(), ServingError> {
        self.validate()?;
        self.verify_resources(table.provided())?;
        let contract = self.contract()?;
        (self.install)(table, self.dependencies, contract)
    }
}

impl<O: Handler> NativeInstance for NativeOperation<O> {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind {
        error.downcast_ref::<O::Error>().map_or(crate::operations::FailureKind::Permanent, O::classify)
    }

    fn initialize(&mut self, context: OperationContext) -> ServiceFuture<'_, ()> {
        Box::pin(async move {
            if self.operation.is_none() {
                let context = context.for_registration(self.dependencies.clone());
                self.operation = Some(O::initialize(context).await.map_err(operation_error::<O>)?);
            }
            Ok(())
        })
    }

    fn run(&mut self, context: OperationContext) -> ServiceFuture<'_, super::response::ErasedResponse> {
        Box::pin(async move {
            let operation = self.operation.as_mut().ok_or(ServingError::Uninitialized)?;
            let context = context.for_registration(self.dependencies.clone());
            #[cfg(feature = "streaming")]
            let attempt = if let Some(streams) = &self.streams {
                self.epoch = self.epoch.checked_add(1).ok_or(ServingError::StreamPhase)?;
                Some(streams.start(self.epoch)?)
            } else {
                None
            };
            let response = operation.run(context, &mut self.request, &mut self.endpoints).await.map_err(operation_error::<O>)?;
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
                use crate::Message;
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
        self.endpoints.transferred() || self.operation.as_ref().is_some_and(Handler::transferred)
    }
}

impl ServiceFactory {
    pub fn initialize(&self, context: OperationContext) -> ServiceFuture<'static, Box<dyn ServiceInstance>> {
        (self.initialize)(context.for_registration(self.dependencies.clone()))
    }
}

impl<S: Service> ServiceInstance for InitializedService<S> {
    fn run(&mut self, context: OperationContext) -> ServiceFuture<'_, ()> {
        Box::pin(async move { self.service.run(context.for_registration(self.dependencies.clone())).await.map_err(service_error) })
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

fn operation_error<O: Handler>(error: O::Error) -> ServingError {
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

    struct Read {
        value: Arc<u64>,
        calls: usize,
    }

    impl OperationDeclaration for Read {
        type Request = ();
        type Response = u64;
        type Input = crate::operations::NoStream;
        type Output = crate::operations::NoStream;
        const ID: &'static str = "read";
        const POLICY: crate::OperationPolicy = crate::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            retries: crate::RetryPolicy::Never,
        };

        fn key(_: &()) -> Option<super::super::CacheKey> {
            Some(super::super::CacheKey::new(vec![42]).unwrap())
        }
    }

    impl Executable for Read {
        type Handler = Self;
    }

    impl Handler for Read {
        type Request = ();
        type Response = u64;
        type Error = crate::ResourceError;
        type StreamEndpoints = ();

        async fn initialize(context: OperationContext) -> Result<Self, Self::Error> {
            Ok(Self {
                value: context.resource::<u64>()?,
                calls: 0,
            })
        }

        async fn run(&mut self, _: OperationContext, _: &mut (), _: &mut ()) -> Result<crate::Response<u64>, Self::Error> {
            self.calls += 1;
            assert!(self.calls <= 2);
            Ok(crate::Response::from_owner(self.value.clone()))
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
        let mut dependencies = Dependencies::new();
        dependencies.insert(value).unwrap();
        let batch = RegistrationBatch::new(dependencies).operation::<Read>();
        let factory = &batch.operations[0];
        assert!(factory.prepare_native(Box::new(7u64), Box::new(())).is_err());
        let mut invocation = factory.prepare_native(Box::new(()), Box::new(())).unwrap();
        let context = OperationContext::new(Dependencies::new(), super::super::CallContext::default());

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

        let mut dependencies = Dependencies::new();
        dependencies.insert(Arc::new(42u64)).unwrap();
        let batch = RegistrationBatch::new(dependencies).operation::<Read>();
        let mut prepared = batch.operations[0].prepare_encoded(&().encode()).unwrap();
        assert_eq!(prepared.key, Read::key(&()));
        let context = OperationContext::new(Dependencies::new(), super::super::CallContext::default());
        ready(prepared.instance.initialize(context.clone()));
        let response = ready(prepared.instance.run(context)).restore::<Vec<u8>>().unwrap();
        assert_eq!(u64::decode(&response).unwrap(), 42);
        ready(prepared.instance.stop());
        ready(prepared.instance.cleanup());
    }

    impl Service for Worker {
        type Error = crate::ResourceError;

        async fn initialize(context: OperationContext) -> Result<Self, Self::Error> {
            let counter = context.resource::<AtomicUsize>()?;
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(Self(counter))
        }

        async fn run(&mut self, _: OperationContext) -> Result<(), Self::Error> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn factory_rejects_a_substituted_declaration() {
        let mut batch = RegistrationBatch::new(Dependencies::new()).operation::<Read>();
        batch.operations[0].definition.id = crate::Identifier::new("different").unwrap();
        assert!(matches!(batch.operations[0].validate(), Err(ServingError::Catalog)));
    }

    #[test]
    fn wire_identity_fences_replacement_before_decoding() {
        use crate::operations::{ServingError, WireTable};
        use crate::{Message, QualifiedId};
        let mut dependencies = Dependencies::new();
        dependencies.insert(Arc::new(42u64)).unwrap();
        let batch = RegistrationBatch::new(dependencies).operation::<Read>();
        let fingerprint = crate::OperationDefinition::of::<Read>().unwrap().fingerprint.0;
        let mut table = WireTable::new("fixture").unwrap();
        table.capacity(std::num::NonZeroUsize::MIN);
        table.register(batch.clone()).unwrap();
        assert!(matches!(table.prepare(1, "spoof.read", fingerprint, 1, ().encode()), Err(ServingError::Unknown { .. })));
        assert!(matches!(table.prepare(1, "fixture.read", [0; 32], 1, vec![255]), Err(ServingError::Identity { .. })));
        assert!(matches!(table.prepare(1, "fixture.read", fingerprint, 0, vec![255]), Err(ServingError::Identity { .. })));
        table.prepare(1, "fixture.read", fingerprint, 7, ().encode()).unwrap();
        ready(Box::pin(table.unregister(&QualifiedId::qualify("fixture", "read").unwrap())));
        table.register(batch).unwrap();
        assert!(matches!(table.prepare(2, "fixture.read", fingerprint, 7, vec![255]), Err(ServingError::Identity { .. })));
        table.prepare(2, "fixture.read", fingerprint, 8, ().encode()).unwrap();
        assert!(matches!(table.prepare(3, "fixture.read", fingerprint, 9, vec![255]), Err(ServingError::Identity { .. })));
        ready(Box::pin(table.settle()));
    }

    #[test]
    fn service_factory_reconstructs_from_its_snapshot() {
        let counter = Arc::new(AtomicUsize::new(0));
        let mut dependencies = Dependencies::new();
        dependencies.insert(counter.clone()).unwrap();
        let policy = ServicePolicy {
            stop_timeout: std::time::Duration::from_secs(1),
            restart: crate::RestartPolicy::Never,
        };
        let batch = RegistrationBatch::new(dependencies).service::<Worker>("worker", policy);
        let factory = &batch.services[0];
        let context = OperationContext::new(Dependencies::new(), super::super::CallContext::default());
        let mut first = factory.initialize(context.clone());
        let mut second = factory.initialize(context);
        let mut task = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(matches!(first.as_mut().poll(&mut task), std::task::Poll::Ready(Ok(_))));
        assert!(matches!(second.as_mut().poll(&mut task), std::task::Poll::Ready(Ok(_))));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
