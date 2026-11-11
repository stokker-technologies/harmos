/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Function Operation Dispatch

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{any::TypeId, future::IntoFuture, marker::PhantomData, sync::Arc};

// Third Party Imports

// Local Imports
use super::*;
use crate::execution::operations::{EndpointSet, FailureKind, Instance, InvocationError, OperationSpec, RequestShape, Response, Shape};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Endpoints<Q> = <(<Q as RequestShape>::Input, <Q as RequestShape>::Output) as Shape>::StreamEndpoints;

/* === Structs === */

pub struct FunctionContext<Q: RequestShape>
where
    (Q::Input, Q::Output): Shape,
    Endpoints<Q>: Sync,
{
    execution: ExecutionContext,
    endpoints: Arc<Endpoints<Q>>,
    resources: &'static [Resource],
    operation: String,
    callees: &'static [WireDeclaration],
}

#[must_use = "an invocation runs when awaited or started"]
pub struct FunctionInvocation<Q: RequestShape, E: EndpointSet = ()> {
    execution: ExecutionContext,
    specification: OperationSpec<Q>,
    request: Q::Request,
    caller: String,
    callees: &'static [WireDeclaration],
    endpoints: E,
    retry: Option<super::super::RetryPolicy>,
}

pub struct FunctionHandle<Q: RequestShape> {
    inner: Box<dyn super::super::ChildInstance>,
    operation: &'static str,
    marker: PhantomData<fn() -> Q>,
}

struct FunctionInstance<Q, H, E>
where
    Q: RequestShape,
    (Q::Input, Q::Output): Shape,
    Endpoints<Q>: Sync,
{
    dependencies: ResourceSet,
    request: Q::Request,
    endpoints: Arc<Endpoints<Q>>,
    resources: &'static [Resource],
    operation: String,
    callees: &'static [WireDeclaration],
    handler: Arc<H>,
    classify: fn(&E) -> FailureKind,
    encoded: bool,
    #[cfg(feature = "grpc")]
    protobuf: bool,

    #[cfg(feature = "streaming")]
    streams: Option<super::super::WireStreams>,

    #[cfg(feature = "streaming")]
    epoch: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<Q: RequestShape> FunctionContext<Q>
where
    (Q::Input, Q::Output): Shape,
    Endpoints<Q>: Sync,
{
    pub fn resource<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, super::super::InvocationError> {
        let declared = self.resources.iter().any(|resource| resource.id() == TypeId::of::<T>());
        if !declared {
            return Err(super::super::InvocationError::Resource {
                operation: self.operation.clone(),
                resource: std::any::type_name::<T>(),
            });
        }
        self.execution.resource::<T>().map_err(|error| super::super::InvocationError::failed(self.operation.clone(), error))
    }

    pub fn streams(&self) -> &Endpoints<Q> {
        &self.endpoints
    }

    pub fn call(&self) -> &super::super::CallContext {
        &self.execution.call
    }

    pub async fn cancelled(&self) {
        self.execution.cancellation.cancelled().await;
    }

    pub fn cancellation_requested(&self) -> bool {
        self.execution.cancellation.is_requested()
    }

    pub fn operation<T: RequestShape>(&self, specification: OperationSpec<T>, request: T::Request) -> FunctionInvocation<T> {
        FunctionInvocation {
            execution: self.execution.clone(),
            specification,
            request,
            caller: self.operation.clone(),
            callees: self.callees,
            endpoints: (),
            retry: None,
        }
    }
}

impl<Q: RequestShape, E: EndpointSet> FunctionInvocation<Q, E> {
    pub fn endpoints<F: EndpointSet>(self, endpoints: F) -> FunctionInvocation<Q, F> {
        FunctionInvocation {
            execution: self.execution,
            specification: self.specification,
            request: self.request,
            caller: self.caller,
            callees: self.callees,
            endpoints,
            retry: self.retry,
        }
    }

    pub fn retry(mut self, retry: super::super::RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    pub async fn start(self) -> Result<FunctionHandle<Q>, InvocationError> {
        let operation = self.specification.declaration.operation.id;
        let target = super::super::Target::request(self.specification).map_err(|error| InvocationError::failed(operation, error))?;
        let declared = self.callees.iter().any(|callee| {
            callee.operation.id == operation
                && (callee.operation.fingerprint)(operation).is_ok_and(|fingerprint| fingerprint == target.definition.fingerprint)
        });
        if !declared {
            return Err(InvocationError::Undeclared { caller: self.caller, operation });
        }
        let calls = self.execution.resource::<super::super::Calls>().map_err(|error| InvocationError::failed(operation, error))?;
        let inner = calls.0.start(target, Box::new(self.request), self.retry, Box::new(self.endpoints), self.execution.call).await?;
        Ok(FunctionHandle { inner, operation, marker: PhantomData })
    }
}

impl<Q: RequestShape, E: EndpointSet> IntoFuture for FunctionInvocation<Q, E> {
    type Output = Result<Response<Q::Response>, InvocationError>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send>>;
    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.start().await?.join().await })
    }
}

impl<Q: RequestShape> Instance<Q> for FunctionHandle<Q> {
    type Error = InvocationError;
    fn id(&self) -> super::super::instance::Id {
        self.inner.id()
    }
    fn status(&self) -> impl std::future::Future<Output = Result<super::super::instance::Status, InvocationError>> + Send {
        self.inner.status()
    }
    fn request_cancel(&self) -> impl std::future::Future<Output = Result<(), InvocationError>> + Send {
        self.inner.request_cancel()
    }
    async fn join(self) -> Result<Response<Q::Response>, InvocationError> {
        self.inner.join().await?.restore::<Q::Response>().ok_or(InvocationError::Response { operation: self.operation })
    }
    fn detach(self) -> super::super::instance::Id {
        self.inner.detach()
    }
}

impl OperationFactory {
    pub fn function<Q, H, F, E>(
        specification: OperationSpec<Q>, resources: &'static [Resource], handler: H, classify: fn(&E) -> FailureKind,
    ) -> Result<Self, ServingError>
    where
        Q: RequestShape,
        Q::Request: Clone,
        (Q::Input, Q::Output): Shape,
        Endpoints<Q>: Sync,
        H: Fn(Q::Request, FunctionContext<Q>) -> F + Send + Sync + 'static,
        F: std::future::Future<Output = Result<Response<Q::Response>, E>> + Send + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        let native_handler = Arc::clone(&handler);
        let definition = specification.declared().map_err(ServingError::Declaration)?;
        let declared = definition.clone();
        let named = specification.declaration.clone();
        let contract = specification.declaration.clone();
        let callees = specification.callees;
        let id = specification.declaration.operation.id;
        let grant = specification.grant;
        Ok(Self {
            definition,
            declared: Arc::new(move || declared.clone()),
            named: Arc::new(move |id| {
                let mut definition = named.operation.definition().map_err(ServingError::Declaration)?;
                definition.id = crate::execution::Identifier::new(id).map_err(harmos_domain::Error::from)?;
                definition.fingerprint = (named.operation.fingerprint)(id).map_err(ServingError::Declaration)?;
                definition.validate().map_err(ServingError::Declaration)?;
                Ok(definition)
            }),
            registered_id: None,
            builtin: false,
            operations: Arc::new(move || {
                callees
                    .iter()
                    .map(|callee| {
                        Ok(super::super::OperationDependency {
                            id: callee.operation.id.to_owned(),
                            fingerprint: (callee.operation.fingerprint)(callee.operation.id).map_err(ServingError::Declaration)?,
                        })
                    })
                    .collect()
            }),
            resources,
            grant: Arc::new(move |registered, grant| {
                grant.map(|mut grant| {
                    if grant.operation() == id && registered != id {
                        grant.operation = registered.to_owned().into();
                    }
                    grant
                })
            }),
            request_grant: Arc::new(move |request| Ok(request.downcast_ref::<Q::Request>().and_then(grant))),
            contract: Arc::new(move |settings| contract.contract_with_settings(settings)),
            native_types: || (TypeId::of::<Q::Request>(), TypeId::of::<Q::Response>(), TypeId::of::<Endpoints<Q>>()),
            endpoints_valid: || true,
            native: Arc::new(move |definition, dependencies, request, endpoints| {
                let request = *request.downcast::<Q::Request>().map_err(|_| ServingError::Catalog)?;
                let endpoints = *endpoints.downcast::<Endpoints<Q>>().map_err(|_| ServingError::Catalog)?;
                Ok(Box::new(FunctionInstance::<Q, H, E> {
                    dependencies,
                    request,
                    endpoints: Arc::new(endpoints),
                    resources,
                    operation: definition.id.to_string(),
                    callees,
                    handler: Arc::clone(&native_handler),
                    classify,
                    encoded: false,
                    #[cfg(feature = "grpc")]
                    protobuf: false,
                    #[cfg(feature = "streaming")]
                    streams: None,
                    #[cfg(feature = "streaming")]
                    epoch: 0,
                }))
            }),
            encoded: Arc::new(move |definition, dependencies, payload, protobuf, limits| {
                #[cfg(not(feature = "streaming"))]
                let _ = limits;
                #[cfg(feature = "streaming")]
                let (endpoints, streams) = match Endpoints::<Q>::empty() {
                    Some(endpoints) => (endpoints, None),
                    None => {
                        let (endpoints, streams) = Endpoints::<Q>::bind_wire(limits.ok_or(ServingError::StreamLimits)?)?;
                        (endpoints, Some(streams))
                    }
                };
                #[cfg(not(feature = "streaming"))]
                let endpoints = Endpoints::<Q>::empty().ok_or(ServingError::StreamUnsupported)?;
                #[cfg(not(feature = "grpc"))]
                if protobuf {
                    return Err(ServingError::StreamUnsupported);
                }
                #[cfg(feature = "grpc")]
                let codec = protobuf.then(super::super::serving::protobuf::Codec::request::<Q>);
                #[cfg(feature = "grpc")]
                let request = if let Some(codec) = codec {
                    let mut request = None::<Q::Request>;
                    (codec.decode)(payload, &mut request).map_err(ServingError::Codec)?;
                    request.ok_or(ServingError::Catalog)?
                } else {
                    Q::Request::decode(payload).map_err(ServingError::Codec)?
                };
                #[cfg(not(feature = "grpc"))]
                let request = Q::Request::decode(payload).map_err(ServingError::Codec)?;
                #[cfg(all(feature = "grpc", feature = "streaming"))]
                let streams = match (streams, codec) {
                    (Some(streams), Some(codec)) => Some(streams.protobuf(codec)?),
                    (streams, _) => streams,
                };
                let grant = grant(&request);
                Ok(PreparedNative {
                    instance: Box::new(FunctionInstance::<Q, H, E> {
                        dependencies,
                        request,
                        endpoints: Arc::new(endpoints),
                        resources,
                        operation: definition.id.to_string(),
                        callees,
                        handler: Arc::clone(&handler),
                        classify,
                        encoded: true,
                        #[cfg(feature = "grpc")]
                        protobuf,
                        #[cfg(feature = "streaming")]
                        streams: streams.clone(),
                        #[cfg(feature = "streaming")]
                        epoch: 0,
                    }),
                    grant,
                    #[cfg(feature = "streaming")]
                    streams,
                })
            }),
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            grpc_memory: |payload| {
                crate::execution::grpc::prepare::<Q::Request>(crate::execution::grpc::DecodeInput::new(payload)).map(|value| value.memory())
            },
            decoding_memory: Q::Request::decoding_memory,
        })
    }
}

impl<Q, H, F, E> NativeInstance for FunctionInstance<Q, H, E>
where
    Q: RequestShape,
    Q::Request: Clone,
    (Q::Input, Q::Output): Shape,
    Endpoints<Q>: Sync,
    H: Fn(Q::Request, FunctionContext<Q>) -> F + Send + Sync + 'static,
    F: std::future::Future<Output = Result<Response<Q::Response>, E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        error.downcast_ref::<E>().map_or(FailureKind::Permanent, self.classify)
    }

    fn initialize(&mut self, _: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, super::super::response::ErasedResponse> {
        Box::pin(async move {
            let execution = context.with_resources(self.dependencies.clone());
            let context = FunctionContext::<Q> {
                execution,
                endpoints: Arc::clone(&self.endpoints),
                resources: self.resources,
                operation: self.operation.clone(),
                callees: self.callees,
            };
            #[cfg(feature = "streaming")]
            let attempt = if let Some(streams) = &self.streams {
                self.epoch = self.epoch.checked_add(1).ok_or(ServingError::StreamPhase)?;
                Some(streams.start(self.epoch)?)
            } else {
                None
            };
            let response = (self.handler)(self.request.clone(), context).await.map_err(|error| ServingError::Native(Arc::new(error)))?;
            #[cfg(feature = "streaming")]
            if let Some(attempt) = attempt {
                attempt.end();
            }
            #[cfg(feature = "grpc")]
            if self.protobuf {
                let mut bytes = Vec::new();
                crate::execution::grpc::encode(&*response, &mut bytes).map_err(ServingError::Codec)?;
                return Ok(Response::new(bytes).into_erased());
            }
            if self.encoded {
                return Ok(Response::new(response.encode()).into_erased());
            }
            Ok(response.into_erased())
        })
    }

    #[cfg(feature = "streaming")]
    fn run_stream(&mut self, context: ExecutionContext, epoch: u64) -> ServiceFuture<'_, super::super::response::ErasedResponse> {
        if self.streams.is_none() || self.epoch.checked_add(1) != Some(epoch) {
            return Box::pin(async { Err(ServingError::StreamPhase) });
        }
        self.run(context)
    }

    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn cleanup(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn transferred(&self) -> bool {
        self.endpoints.transferred()
    }

    #[cfg(feature = "streaming")]
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        self.endpoints.progress()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
