/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private In-Process Operations

/* === Modules === */

#[cfg(feature = "streaming")]
mod streaming {
    use super::*;
    use crate::execution::operations::{StreamError, StreamInput, StreamOutput, StreamSettings, stream};

    pub struct NativeOutput<T>(StreamOutput<T>);

    impl<T: MemorySize + Send + 'static> EndpointSet for NativeOutput<T> {
        fn item_types() -> (Option<TypeId>, Option<TypeId>) {
            (None, Some(TypeId::of::<T>()))
        }
        fn transferred(&self) -> bool {
            self.0.transfer_flag().transferred()
        }
        fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
            vec![self.0.transfer_flag().subscribe()]
        }
    }
    impl<T: MemorySize> NativeOutput<T> {
        pub async fn send(&self, value: T) -> Result<(), StreamError> {
            self.0.send_with(value.memory_size(), || Ok(value)).await
        }
    }
    impl<T> Clone for NativeOutput<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    pub fn native_stream<T>(settings: StreamSettings) -> Result<(NativeOutput<T>, StreamInput<T>), StreamError> {
        let (output, input) = stream(settings)?;
        Ok((NativeOutput(output), input))
    }
}

/* === Re-Exports === */

#[cfg(feature = "streaming")]
pub use streaming::{NativeOutput, native_stream};

/* === Imports === */

// Standard Library Imports
use std::{any::TypeId, future::Future, marker::PhantomData, sync::Arc};

// Third Party Imports

// Local Imports
use super::{NativeInstance, OperationFactory, ServiceFuture};
use crate::execution::operations::{
    Calls, EndpointSet, ExecutionContext, OperationDefinition, OperationPolicy, ReplaySafety, Resource, ResourceSet, Response,
    ServingError, Target, Visibility,
};
use crate::{Fingerprint, Identifier, MemorySize, MessageError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Instance<O: NativeOperation> {
    request: O::Request,
    endpoints: O::Endpoints,
    resources: ResourceSet,
    marker: PhantomData<O>,
}

/* === Enums === */

/* === Traits === */

/// A Private Operation Whose Payloads Remain In This Process.
pub trait NativeHost: Sync {
    type Error: Send;
    fn call_native<O: NativeOperation<Endpoints = ()>>(
        &self, request: O::Request,
    ) -> impl Future<Output = Result<O::Response, Self::Error>> + Send;
}

pub trait NativeOperation: Send + Sync + 'static {
    type Request: Clone + MemorySize + Send + Sync + 'static;
    type Response: Send + Sync + 'static;
    type Endpoints: EndpointSet;
    const ID: &'static str;
    const POLICY: OperationPolicy;
    const REPLAY: ReplaySafety = ReplaySafety::Unknown;
    const RESOURCES: &'static [Resource] = &[];
    fn classify(_: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind {
        crate::operations::FailureKind::Permanent
    }
    fn run(
        context: ExecutionContext, request: Self::Request, endpoints: &Self::Endpoints,
    ) -> impl Future<Output = Result<Self::Response, ServingError>> + Send;
}

/* === Implementations === */

impl Target {
    pub fn native<O: NativeOperation>() -> Result<Self, ServingError> {
        let factory = OperationFactory::native::<O>()?;
        Ok(Self {
            in_process: true,
            id: O::ID,
            definition: factory.definition,
            protobuf_native: false,
            request: TypeId::of::<O::Request>,
            response: TypeId::of::<O::Response>,
            memory: |request| request.downcast_ref::<O::Request>().map_or(0, MemorySize::memory_size),
            encode: |_| None,
            decode: |_| Err(MessageError::borrowed("Read Native Response: payload has no wire representation")),
            #[cfg(feature = "grpc")]
            encode_protobuf: |_| Err(MessageError::borrowed("Encode Native Request: payload has no wire representation")),
            #[cfg(feature = "grpc")]
            decode_protobuf: |_| Err(MessageError::borrowed("Decode Native Response: payload has no wire representation")),
        })
    }
}
impl ExecutionContext {
    pub async fn call_native<O: NativeOperation<Endpoints = ()>>(
        &self, request: O::Request,
    ) -> Result<Response<O::Response>, ServingError> {
        let target = Target::native::<O>()?;
        let calls = self.resource::<Calls>().map_err(|_| ServingError::Catalog)?;
        let child = calls.0.start(target, Box::new(request), None, Box::new(()), self.call.clone()).await?;
        child.join().await?.restore().ok_or(ServingError::EndpointTypes)
    }
}
impl OperationFactory {
    pub fn native<O: NativeOperation>() -> Result<Self, ServingError> {
        let declared = definition::<O>(O::ID)?;
        Ok(Self {
            definition: declared.clone(),
            operations: Arc::new(|| Ok(Vec::new())),
            declared: Arc::new(move || declared.clone()),
            named: Arc::new(definition::<O>),
            registered_id: None,
            grant: Arc::new(|_, _| None),
            request_grant: Arc::new(|_| Ok(None)),
            builtin: false,
            resources: O::RESOURCES,
            contract: Arc::new(|_| Err(crate::operations::ContractError::Codec { codec: "harmos.native".into() })),
            native: Arc::new(|_, resources, request, endpoints| {
                Ok(Box::new(Instance::<O> {
                    request: *request.downcast().map_err(|_| ServingError::EndpointTypes)?,
                    endpoints: *endpoints.downcast().map_err(|_| ServingError::EndpointTypes)?,
                    resources,
                    marker: PhantomData,
                }))
            }),
            native_types: || (TypeId::of::<O::Request>(), TypeId::of::<O::Response>(), TypeId::of::<O::Endpoints>()),
            endpoints_valid: || true,
            encoded: Arc::new(|_, _, _, _, _| Err(ServingError::StreamUnsupported)),
            #[cfg(all(feature = "grpc", feature = "streaming"))]
            grpc_memory: |_| Err(MessageError::borrowed("Decode Native Request: payload has no wire representation")),
            decoding_memory: |_| Err(MessageError::borrowed("Decode Native Request: payload has no wire representation")),
        })
    }
}
impl<O: NativeOperation> NativeInstance for Instance<O> {
    fn classify(&self, error: &(dyn std::error::Error + Send + Sync + 'static)) -> crate::operations::FailureKind {
        O::classify(error)
    }
    fn initialize(&mut self, _: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn run(&mut self, context: ExecutionContext) -> ServiceFuture<'_, crate::execution::__private::ErasedResponse> {
        Box::pin(async move {
            let context = context.with_resources(self.resources.clone());
            O::run(context, self.request.clone(), &self.endpoints).await.map(|value| Response::new(value).into_erased())
        })
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

fn definition<O: NativeOperation>(id: &str) -> Result<OperationDefinition, ServingError> {
    // Native Types Are Matched By TypeId; This Identity Is Never A Wire Contract.
    let fingerprint = crate::schema::identity::operation(id, Fingerprint([0; 32]), Fingerprint([0; 32]), None, None);
    let definition = OperationDefinition {
        id: Identifier::new(id).map_err(crate::operations::Error::from)?,
        description: String::new(),
        visibility: Visibility::Private,
        policy: O::POLICY,
        replay: O::REPLAY,
        fingerprint,
    };
    definition.validate()?;
    Ok(definition)
}

/* === Tests === */
