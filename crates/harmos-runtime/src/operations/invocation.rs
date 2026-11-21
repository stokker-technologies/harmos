/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Declared Calls Preserve Typed Payload Owners Through Erased Admission.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::any::Any;
use std::sync::{Arc, Mutex};

// Third Party Imports

// Local Imports
use super::{HostContext, Response};
use futures_util::future::BoxFuture;
use harmos_domain::{MemorySize, Message, MessageError};
use harmos_operations::execution::__private::ErasedResponse;
use harmos_operations::execution::operations::{
    CallFuture, Caller, ChildInstance, ControlFuture, InvocationError, NativeOperation, OperationFactory, StartFuture, Target,
};

/* === Constants === */

/* === Macros === */

macro_rules! native_payload {
    ($ty:ty) => {
        impl Message for $ty {
            const CODEC: &'static str = "harmos.native";
            fn encode(&self) -> Vec<u8> {
                panic!("native payload cannot be encoded")
            }
            fn encoded_size(&self) -> usize {
                0
            }
            fn decode(_: &[u8]) -> Result<Self, MessageError> {
                Err(MessageError::borrowed("native payload cannot be decoded"))
            }
            fn decoding_memory(_: &[u8]) -> Result<usize, MessageError> {
                Err(MessageError::borrowed("native payload cannot be decoded"))
            }
        }
        impl harmos_domain::schema::Described for $ty {
            const NAME: &'static str = "harmos::native";
            const FIELDS: &'static [harmos_domain::schema::declaration::Field] = &[];
            const MARKER: &'static [u8] = &[];
        }
    };
}

native_payload!(Request);

native_payload!(Answer);

/* === Type Aliases === */

/* === Structs === */

pub struct NativeInvocation<O: NativeOperation> {
    id: super::OperationInstanceId,
    future: BoxFuture<'static, crate::Result<Response<O::Response>>>,
    control: Arc<crate::supervision::instance::Instance>,
}

struct NativeRequest<'a, P> {
    target: &'a Target,
    request: Box<dyn Any + Send>,
    retry: Option<super::RetryPolicy>,
    policy: Option<(super::OperationPolicy, &'static str)>,
    endpoints: P,
    factory: OperationFactory,
    erase: fn(P) -> Box<dyn Any + Send>,
}

struct Request {
    payload: Mutex<Option<Box<dyn Any + Send>>>,
    memory: usize,
}

struct Answer(ErasedResponse);

struct NativeCall;

struct NativeChild(super::Running<NativeCall>, Target);

struct EncodedChild {
    invocation: Option<super::EncodedInvocation>,
    target: Target,
    protobuf: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<O: NativeOperation> std::future::Future for NativeInvocation<O> {
    type Output = crate::Result<super::Response<O::Response>>;
    fn poll(mut self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        self.future.as_mut().poll(context)
    }
}
impl<O: NativeOperation> NativeInvocation<O> {
    pub fn id(&self) -> super::OperationInstanceId {
        self.id
    }

    pub fn request_cancel(&self) {
        self.control.request_cancel();
    }
}
impl super::Manager {
    pub fn submit_native<O: NativeOperation>(&self, request: O::Request, endpoints: O::Endpoints) -> crate::Result<NativeInvocation<O>> {
        self.submit_native_as::<O>(request, endpoints, super::caller::Source::Host, None)
    }
    /// Submit as Host With `policy` Replacing the Operation's Policy for This Invocation; Its Source Names It.
    pub(crate) fn submit_native_with<O: NativeOperation<Endpoints = ()>>(
        &self, request: O::Request, policy: (super::OperationPolicy, &'static str),
    ) -> crate::Result<NativeInvocation<O>> {
        let policy = Some(policy);
        self.submit_native_as::<O>(request, (), super::caller::Source::Host, policy)
    }
    pub(super) fn submit_native_as<O: NativeOperation>(
        &self, request: O::Request, endpoints: O::Endpoints, source: super::caller::Source,
        policy: Option<(super::OperationPolicy, &'static str)>,
    ) -> crate::Result<NativeInvocation<O>> {
        self.submit_native_scoped::<O>(request, endpoints, source, policy, None)
    }

    pub(crate) fn submit_native_in<O: NativeOperation>(
        &self, request: O::Request, endpoints: O::Endpoints, policy: Option<(super::OperationPolicy, &'static str)>,
        parent: Option<&super::Context<'_>>,
    ) -> crate::Result<NativeInvocation<O>> {
        let source = parent.map_or(super::caller::Source::Host, |parent| parent.source.native());
        self.submit_native_scoped::<O>(request, endpoints, source, policy, parent)
    }

    fn submit_native_scoped<O: NativeOperation>(
        &self, request: O::Request, endpoints: O::Endpoints, source: super::caller::Source,
        policy: Option<(super::OperationPolicy, &'static str)>, parent: Option<&super::Context<'_>>,
    ) -> crate::Result<NativeInvocation<O>> {
        let target = Target::native::<O>().map_err(|source| crate::Error::OperationFailed {
            operation: O::ID.into(),
            source: Arc::new(source),
        })?;
        let entry = self.entry_as(O::ID, &source.check()?)?;
        let factory = entry.binding.downcast_ref::<OperationFactory>().cloned().ok_or(crate::Error::InvalidInput {
            field: "Call Native Operation",
            reason: "native factory required",
        })?;
        if !factory.accepts_target(&target, std::any::TypeId::of::<O::Endpoints>()) {
            return Err(crate::Error::InvalidInput {
                field: "Call Native Operation",
                reason: "request or endpoints differ from the registered operation",
            });
        }
        let running = admit_native_as(
            self,
            NativeRequest {
                target: &target,
                request: Box::new(request),
                retry: None,
                policy,
                endpoints,
                factory,
                erase: |endpoints| Box::new(endpoints),
            },
            &source,
            parent.map(|parent| &parent.scope),
            parent.map_or(&[], |parent| parent.admission.held()),
        )?;
        let id = running.id();
        let control = running.control.clone();
        let future = Box::pin(async move {
            let response = crate::clock::waiting(running).await?.0.clone();
            response.restore().ok_or(crate::Error::InvalidInput {
                field: "Call Native Operation",
                reason: "native response type differs",
            })
        });
        Ok(NativeInvocation { id, future, control })
    }
    pub async fn call_native<O: NativeOperation<Endpoints = ()>>(&self, request: O::Request) -> crate::Result<O::Response> {
        let response = self.submit_native::<O>(request, ())?.await?;
        response.try_unwrap().map_err(|_| crate::Error::InvalidInput {
            field: "Call Native Operation",
            reason: "native response is shared",
        })
    }
}

impl Caller for HostContext {
    fn start(
        &self, target: Target, request: Box<dyn Any + Send>, retry: Option<super::RetryPolicy>, endpoints: Box<dyn Any + Send>,
        call: super::CallContext,
    ) -> StartFuture<'_> {
        Box::pin(async move {
            let id = target.id;
            let result = async {
                let mut context = self.borrow();
                context.operations = context.operations.with_context(call);
                // Native Admission Retains the Wrapper's Authority and Its Originating Caller.
                if target.in_process {
                    context.source = context.source.native();
                }
                let origin = context.source.check()?;
                let entry = context.operations.entry_as(target.id, &origin)?;
                if !target.in_process && entry.declared().fingerprint != target.definition.fingerprint {
                    return Err(crate::Error::InvalidInput {
                        field: "declared operation",
                        reason: "callee fingerprint differs",
                    });
                }
                let Some(factory) = entry.binding.downcast_ref::<OperationFactory>().cloned() else {
                    let protobuf = entry.binding.is::<crate::plugin::grpc::invoke::Binding>();
                    let payload = if protobuf {
                        (target.encode_protobuf)(request.as_ref())
                            .map_err(|source| crate::Error::OperationPayload { operation: target.id.into(), source })?
                    } else {
                        (target.encode)(request.as_ref()).ok_or(crate::Error::InvalidInput {
                            field: "declared operation",
                            reason: "callee payload differs",
                        })?
                    };
                    if !endpoints.is::<()>() {
                        return Err(crate::Error::InvalidInput {
                            field: "operation endpoints",
                            reason: "encoded children do not accept native endpoints",
                        });
                    }
                    let invocation = context.submit_encoded_target(entry, payload, retry)?;
                    return Ok(Box::new(EncodedChild {
                        invocation: Some(invocation),
                        target,
                        protobuf,
                    }) as Box<dyn ChildInstance>);
                };
                if !factory.accepts_target(&target, endpoints.as_ref().type_id()) {
                    return Err(crate::Error::InvalidInput {
                        field: "declared operation",
                        reason: "callee payload types differ",
                    });
                }
                let running = if endpoints.is::<()>() {
                    admit_native(&context, &target, request, retry, (), factory, |endpoints| Box::new(endpoints))?
                } else {
                    admit_native(&context, &target, request, retry, endpoints, factory, |endpoints| endpoints)?
                };
                Ok(Box::new(NativeChild(running, target)) as Box<dyn ChildInstance>)
            }
            .await;
            result.map_err(|error: crate::Error| InvocationError::failed(id, error))
        })
    }
}

impl ChildInstance for NativeChild {
    fn id(&self) -> super::OperationInstanceId {
        self.0.id()
    }
    fn status(&self) -> ControlFuture<'_, harmos_operations::execution::operations::instance::Status> {
        Box::pin(std::future::ready(Ok(self.0.control.status().portable())))
    }
    fn request_cancel(&self) -> ControlFuture<'_, ()> {
        self.0.request_cancel();
        Box::pin(async { Ok(()) })
    }
    fn join(self: Box<Self>) -> CallFuture<'static> {
        Box::pin(async move {
            let response = crate::clock::waiting(self.0).await.map_err(|error| InvocationError::failed(self.1.id, error))?.0.clone();
            if self.1.protobuf_native
                && let Some(bytes) = response.clone().restore::<Vec<u8>>()
            {
                return (self.1.decode_protobuf)(&bytes).map_err(|error| InvocationError::failed(self.1.id, error));
            }
            Ok(response)
        })
    }
    fn detach(self: Box<Self>) -> super::OperationInstanceId {
        self.0.detach()
    }
}

impl ChildInstance for EncodedChild {
    fn id(&self) -> super::OperationInstanceId {
        self.invocation.as_ref().expect("owned child").id()
    }
    fn status(&self) -> ControlFuture<'_, harmos_operations::execution::operations::instance::Status> {
        Box::pin(std::future::ready(Ok(self.invocation.as_ref().expect("owned child").control.status().portable())))
    }
    fn request_cancel(&self) -> ControlFuture<'_, ()> {
        self.invocation.as_ref().expect("owned child").request_cancel();
        Box::pin(async { Ok(()) })
    }
    fn join(mut self: Box<Self>) -> CallFuture<'static> {
        Box::pin(async move {
            let invocation = self.invocation.take().expect("owned child");
            // Keep Cancellation On Abandonment While Awaiting The Encoded Handle.
            let control = invocation.control.clone();
            struct Cancel(Option<std::sync::Arc<crate::supervision::instance::Instance>>);
            impl Drop for Cancel {
                fn drop(&mut self) {
                    if let Some(control) = &self.0 {
                        control.request_cancel();
                    }
                }
            }
            let mut cancel = Cancel(Some(control));
            let answer = crate::clock::waiting(invocation).await;
            cancel.0.take();
            let bytes = answer.map_err(|error| InvocationError::failed(self.target.id, error))?;
            let decode = if self.protobuf { self.target.decode_protobuf } else { self.target.decode };
            decode(&bytes).map_err(|error| InvocationError::failed(self.target.id, error))
        })
    }
    fn detach(mut self: Box<Self>) -> super::OperationInstanceId {
        self.invocation.take().expect("owned child").id()
    }
}

impl Drop for EncodedChild {
    fn drop(&mut self) {
        if let Some(invocation) = &self.invocation {
            invocation.request_cancel();
        }
    }
}

impl harmos_operations::operations::RequestShape for NativeCall {
    type Request = Request;
    type Response = Answer;
    type Input = harmos_operations::operations::NoStream;
    type Output = harmos_operations::operations::NoStream;
}

impl MemorySize for Request {
    fn memory_size(&self) -> usize {
        self.memory
    }
}

impl MemorySize for Answer {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/* === Public Functions === */

impl harmos_operations::execution::operations::NativeHost for super::Manager {
    type Error = crate::Error;
    async fn call_native<O: NativeOperation<Endpoints = ()>>(&self, request: O::Request) -> crate::Result<O::Response> {
        super::Manager::call_native::<O>(self, request).await
    }
}
impl harmos_operations::execution::operations::NativeHost for super::OperationManager {
    type Error = crate::Error;
    async fn call_native<O: NativeOperation<Endpoints = ()>>(&self, request: O::Request) -> crate::Result<O::Response> {
        super::Manager::call_native::<O>(self, request).await
    }
}
impl harmos_operations::execution::operations::NativeHost for HostContext {
    type Error = crate::Error;
    async fn call_native<O: NativeOperation<Endpoints = ()>>(&self, request: O::Request) -> crate::Result<O::Response> {
        let target = Target::native::<O>().map_err(|source| crate::Error::OperationFailed {
            operation: O::ID.into(),
            source: Arc::new(source),
        })?;
        let child = harmos_operations::execution::operations::Caller::start(
            self,
            target,
            Box::new(request),
            None,
            Box::new(()),
            self.borrow().context().clone(),
        )
        .await
        .map_err(|source| crate::Error::OperationFailed {
            operation: O::ID.into(),
            source: Arc::new(source),
        })?;
        let response = child.join().await.map_err(|source| crate::Error::OperationFailed {
            operation: O::ID.into(),
            source: Arc::new(source),
        })?;
        let response = response.restore::<O::Response>().ok_or(crate::Error::InvalidInput {
            field: "Call Native Operation",
            reason: "response type differs",
        })?;
        response.try_unwrap().map_err(|_| crate::Error::InvalidInput {
            field: "Call Native Operation",
            reason: "response is shared",
        })
    }
}
impl harmos_operations::execution::operations::NativeHost for super::Context<'_> {
    type Error = crate::Error;
    async fn call_native<O: NativeOperation<Endpoints = ()>>(&self, request: O::Request) -> crate::Result<O::Response> {
        harmos_operations::execution::operations::NativeHost::call_native::<O>(&self.owned(), request).await
    }
}

/* === Private Functions === */

fn admit_native<P: Send + 'static>(
    context: &super::Context<'_>, target: &Target, request: Box<dyn Any + Send>, retry: Option<super::RetryPolicy>, endpoints: P,
    factory: OperationFactory, erase: fn(P) -> Box<dyn Any + Send>,
) -> crate::Result<super::Running<NativeCall>> {
    admit_native_as(
        &context.operations,
        NativeRequest {
            target,
            request,
            retry,
            policy: None,
            endpoints,
            factory,
            erase,
        },
        &context.source,
        Some(&context.scope),
        context.admission.held(),
    )
}

fn admit_native_as<P: Send + 'static>(
    manager: &super::Manager, request: NativeRequest<'_, P>, source: &super::caller::Source, scope: Option<&super::Scope<'_>>,
    ancestors: &[crate::operations::lane::Admission],
) -> crate::Result<super::Running<NativeCall>> {
    let NativeRequest {
        target,
        request,
        retry,
        policy,
        endpoints,
        factory,
        erase,
    } = request;
    let grant =
        factory.request_grant(request.as_ref()).map_err(|source| crate::Error::OperationPayload { operation: target.id.into(), source })?;
    let memory = (target.memory)(request.as_ref());
    let entry = manager.entry_as(target.id, &source.check()?)?;
    let resources = manager.resources_for(&entry);
    let binding = super::authored::factory_projected::<NativeCall, P>(
        manager,
        move |request, endpoints| {
            let payload = request
                .payload
                .into_inner()
                .map_err(|_| harmos_operations::execution::ServingError::Catalog)?
                .ok_or(harmos_operations::execution::ServingError::Catalog)?;
            factory.prepare_native(resources.clone(), payload, erase(endpoints))
        },
        |response| Some(harmos_operations::execution::Response::new(Answer(response))),
    );
    let mut input = super::binding::Request::from(Request {
        payload: Mutex::new(Some(request)),
        memory,
    });
    input.retry = retry;
    input.policy = policy.map(|(policy, _)| policy);
    input.policy_source = policy.map(|(_, source)| source);
    input.binding = Some(Arc::new(binding));
    input.grant = grant;
    let running = manager.admit_at::<NativeCall, P>(target.id, (input, endpoints), scope, ancestors, source, None)?;
    Ok(running)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    #[cfg(feature = "grpc")]
    mod caller_relative {
        use std::error;
        use std::num::NonZeroUsize;
        use std::sync::Arc;

        use prost::Message;
        use prost_reflect::ServiceDescriptor;
        use tokio::sync::Notify;

        use super::{Model, POLICY};
        use crate::operations::caller::Source;
        use crate::operations::{
            CallContext, ExecutionContext, HostContext, Manager, NativeOperation, OperationFactory, ProtobufOperation, RegistrationExt,
            Resource, ServingError, StreamSettings, Visibility, native_stream,
        };
        use crate::{Error, Runtime};
        use harmos_conformance::product;
        use harmos_operations::host::declarations::{host, lifecycle};
        use harmos_protocol::host::{
            InvocationsCancelRequest, InvocationsCancelResponse, InvocationsGetRequest, InvocationsGetResponse, InvocationsWatchRequest,
            InvocationsWatchResponse, OperationsGetRequest, OperationsGetResponse, OperationsListRequest, OperationsListResponse,
        };
        use harmos_protocol::model::Callers;

        #[derive(Default)]
        struct Gate {
            started: Notify,
            release: Notify,
        }

        struct Waiting;

        impl NativeOperation for Waiting {
            type Request = ();
            type Response = ();
            type Endpoints = ();
            const ID: &'static str = "fixture.waiting";
            const POLICY: crate::OperationPolicy = POLICY;
            const RESOURCES: &'static [Resource] = &[Resource::of::<Gate>()];

            async fn run(context: ExecutionContext, _: (), _: &()) -> Result<(), ServingError> {
                let gate = context.resource::<Gate>().unwrap();
                gate.started.notify_one();
                gate.release.notified().await;
                Ok(())
            }
        }

        macro_rules! wrapper {
            ($name:ident, $reference:ty, $request:ty, $response:ty, $context:ident, $value:ident, $body:block) => {
                struct $name;
                impl ProtobufOperation for $name {
                    type Request = $request;
                    type Response = $response;
                    type Endpoints = ();
                    const ID: &'static str = <$reference as ProtobufOperation>::ID;
                    const POLICY: crate::OperationPolicy = POLICY;
                    const VISIBILITY: Visibility = Visibility::Public;
                    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];

                    fn service() -> Option<ServiceDescriptor> {
                        <$reference as ProtobufOperation>::service()
                    }

                    async fn run($context: ExecutionContext, $value: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
                        let result: Result<Self::Response, Error> = async $body.await;
                        result.map_err(|error| ServingError::Native(Arc::new(error)))
                    }
                }
            };
        }

        wrapper!(List, product::OperationsList, OperationsListRequest, OperationsListResponse, context, request, {
            let parent = context.resource::<HostContext>().unwrap();
            parent.for_call().call::<host::List>(request).await
        });

        wrapper!(Get, product::OperationsGet, OperationsGetRequest, OperationsGetResponse, context, request, {
            let parent = context.resource::<HostContext>().unwrap();
            parent.for_call().call::<host::Get>(request).await
        });

        wrapper!(GetInvocation, product::InvocationsGet, InvocationsGetRequest, InvocationsGetResponse, context, request, {
            let parent = context.resource::<HostContext>().unwrap();
            parent.for_call().invocations().get(request).await
        });

        wrapper!(CancelInvocation, product::InvocationsCancel, InvocationsCancelRequest, InvocationsCancelResponse, context, request, {
            let parent = context.resource::<HostContext>().unwrap();
            parent.for_call().invocations().cancel(request).await
        });

        wrapper!(WatchInvocation, product::InvocationsWatch, InvocationsWatchRequest, InvocationsWatchResponse, context, request, {
            let parent = context.resource::<HostContext>().unwrap();
            let (output, _input) = native_stream(StreamSettings {
                items: NonZeroUsize::new(4).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            })
            .unwrap();
            let response = parent.for_call().invocations().watch(request, output)?.await?;
            response.try_unwrap().map_err(|_| Error::InvalidInput {
                field: "Watch Invocation",
                reason: "response is shared",
            })
        });

        struct NestedList;

        impl NativeOperation for NestedList {
            type Request = OperationsListRequest;
            type Response = OperationsListResponse;
            type Endpoints = ();
            const ID: &'static str = "fixture.nested_list";
            const POLICY: crate::OperationPolicy = POLICY;

            async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
                let response = context.call_native::<host::List>(request).await?;
                response.try_unwrap().map_err(|_| ServingError::Catalog)
            }
        }

        async fn runtime() -> (Runtime<Model>, Arc<Gate>) {
            let gate = Arc::new(Gate::default());
            let runtime = Runtime::<Model>::start_with((), (), |setup| {
                setup.resource::<Gate>(gate.clone());
                setup.settings().operation_settings(crate::operations::Settings {
                    preparation_bytes: NonZeroUsize::new(65536),
                    ..Default::default()
                });
            })
            .await
            .unwrap();
            runtime.operations.register_catalog(harmos_operations::host_catalog(), Default::default()).await.unwrap();
            runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
            let batch = harmos_operations::execution::__private::Catalog::new()
                .entry(OperationFactory::protobuf::<List>().unwrap())
                .entry(OperationFactory::protobuf::<Get>().unwrap())
                .entry(OperationFactory::protobuf::<GetInvocation>().unwrap())
                .entry(OperationFactory::protobuf::<CancelInvocation>().unwrap())
                .entry(OperationFactory::protobuf::<WatchInvocation>().unwrap())
                .entry(OperationFactory::native::<Waiting>().unwrap())
                .entry(OperationFactory::native::<NestedList>().unwrap());
            runtime.register_batch(batch.lane(Default::default())).await.unwrap();
            (runtime, gate)
        }

        async fn invoke<O: ProtobufOperation>(manager: &Manager, source: &Source, request: O::Request) -> crate::Result<O::Response> {
            let bytes = manager.submit_encoded_as(O::ID, request.encode_to_vec(), source.clone(), None)?.await?;
            Ok(O::Response::decode(bytes.as_slice()).unwrap())
        }

        #[tokio::test(start_paused = true)]
        async fn facade_discovery_retains_client_audience_and_host_authority() {
            let (runtime, _) = runtime().await;
            let manager = runtime.operations.execution();
            let source = Source::Host.external().unwrap();

            // Admit the Declared Client Wrapper and Filter Its Native Result
            let entry = manager.entry(List::ID).unwrap();
            assert_eq!(entry.contract.as_ref().unwrap().callers(), Callers::Clients);
            let listed = invoke::<List>(&manager, &source, OperationsListRequest {}).await.unwrap();
            assert!(listed.operations.iter().any(|entry| entry.contract.as_ref().unwrap().id == List::ID));
            assert!(listed.operations.iter().all(|entry| entry.contract.as_ref().unwrap().id != "harmos.plugin.v1.Time/Get"));
            let hidden =
                invoke::<Get>(&manager, &source, OperationsGetRequest { id: "harmos.plugin.v1.Time/Get".into() }).await.unwrap_err();
            let mut cause = &hidden as &(dyn error::Error + Send + Sync);
            while let Some(Error::OperationFailed { source, .. }) = cause.downcast_ref::<Error>() {
                cause = source.as_ref();
            }
            assert!(
                matches!(
                    cause.downcast_ref::<harmos_operations::Error>(),
                    Some(harmos_operations::Error::NotFound { entity: "operation" })
                ),
                "{hidden:?}"
            );

            // Host Calls Still See the Registered Plugin Operation
            let call = runtime.for_call(&CallContext::default());
            let listed = call.call::<host::List>(OperationsListRequest {}).await.unwrap();
            assert!(listed.operations.iter().any(|entry| entry.contract.as_ref().unwrap().id == "harmos.plugin.v1.Time/Get"));
            call.call::<host::Get>(OperationsGetRequest { id: "harmos.plugin.v1.Time/Get".into() }).await.unwrap();

            // Nested Native Calls Retain the Same Incoming Caller
            let listed =
                manager.submit_native_as::<NestedList>(OperationsListRequest {}, (), source.native(), None).unwrap().await.unwrap();
            assert!(listed.operations.iter().any(|entry| entry.contract.as_ref().unwrap().id == List::ID));
            assert!(listed.operations.iter().all(|entry| entry.contract.as_ref().unwrap().id != "harmos.plugin.v1.Time/Get"));
            runtime.stop().await.unwrap();
        }

        #[tokio::test(start_paused = true)]
        async fn facade_controls_hide_other_callers_and_preserve_delegated_ownership() {
            let (runtime, gate) = runtime().await;
            let manager = runtime.operations.execution();
            let owner = Source::Host.external().unwrap();
            let other = Source::Host.external().unwrap();

            // Native Work Belongs to the Incoming Caller
            let waiting = manager.submit_native_as::<Waiting>((), (), owner.native(), None).unwrap();
            gate.started.notified().await;
            let instance = waiting.id().value();
            invoke::<GetInvocation>(&manager, &owner, InvocationsGetRequest { instance }).await.unwrap();
            let (output, input) = native_stream::<InvocationsWatchResponse>(StreamSettings {
                items: NonZeroUsize::new(4).unwrap(),
                bytes: NonZeroUsize::new(65536).unwrap(),
            })
            .unwrap();
            let watch =
                manager.submit_native_as::<lifecycle::Watch>(InvocationsWatchRequest { instance }, output, owner.native(), None).unwrap();
            assert_eq!(input.recv().await.unwrap().invocation.as_ref().unwrap().phase, "running");

            // Another Caller Cannot Inspect, Cancel or Watch It
            assert!(invoke::<GetInvocation>(&manager, &other, InvocationsGetRequest { instance }).await.is_err());
            assert!(
                invoke::<CancelInvocation>(&manager, &other, InvocationsCancelRequest { instance, reason: String::new() }).await.is_err()
            );
            assert!(invoke::<WatchInvocation>(&manager, &other, InvocationsWatchRequest { instance }).await.is_err());
            assert!(runtime.for_call(&CallContext::default()).invocations().get(InvocationsGetRequest { instance }).await.is_err());
            invoke::<CancelInvocation>(&manager, &owner, InvocationsCancelRequest { instance, reason: String::new() }).await.unwrap();
            assert!(waiting.await.is_err());
            assert_eq!(watch.await.unwrap().invocation.as_ref().unwrap().phase, "settled");

            // Host Work Remains Host-Owned Across a Client Wrapper
            let waiting = runtime.for_call(&CallContext::default()).submit::<Waiting>((), ()).unwrap();
            gate.started.notified().await;
            let instance = waiting.id().value();
            assert!(invoke::<GetInvocation>(&manager, &owner, InvocationsGetRequest { instance }).await.is_err());
            assert!(
                invoke::<CancelInvocation>(&manager, &owner, InvocationsCancelRequest { instance, reason: String::new() }).await.is_err()
            );
            assert!(invoke::<WatchInvocation>(&manager, &owner, InvocationsWatchRequest { instance }).await.is_err());
            runtime.for_call(&CallContext::default()).call::<lifecycle::Get>(InvocationsGetRequest { instance }).await.unwrap();
            gate.release.notify_one();
            waiting.await.unwrap();
            runtime.stop().await.unwrap();
        }
    }

    use super::*;
    use crate::operations::RegistrationExt;
    use harmos_operations::execution::Response;

    struct Model;
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), setup: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            setup.settings().lanes(
                crate::operations::LaneSet::new([crate::operations::Lane::new("Serial", crate::operations::Capacity::serial())]).unwrap(),
            );
            Ok(Self)
        }
    }

    impl crate::Application for Model {
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
    }

    const POLICY: crate::OperationPolicy = crate::OperationPolicy {
        timeout: std::time::Duration::from_secs(2),
        attempt_timeout: std::time::Duration::from_secs(2),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: crate::RetryPolicy::Never,
    };

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Leaf {
        value: u64,
    }

    static LEAF_DESCRIPTION: crate::operations::WireDeclaration = crate::operations::WireDeclaration::request::<Leaf>("test.leaf", POLICY);

    const LEAF_OPERATION: crate::operations::OperationSpec<Leaf> = crate::operations::OperationSpec::<Leaf>::new(&LEAF_DESCRIPTION);
    async fn leaf_handler(request: Leaf, ctx: crate::operations::FunctionContext<Leaf>) -> Result<Response<u64>, std::convert::Infallible> {
        Ok((request.value + *ctx.resource::<u64>().expect("declared resource is registered")).into())
    }

    fn leaf_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(LEAF_OPERATION, const { &[crate::Resource::of::<u64>()] }, leaf_handler, |_| {
            crate::operations::FailureKind::Permanent
        })
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Parent {
        value: u64,
    }

    static PARENT_DESCRIPTION: crate::operations::WireDeclaration =
        crate::operations::WireDeclaration::request::<Parent>("test.parent", POLICY);

    static PARENT_CALLEES: [crate::operations::WireDeclaration; 1] = [LEAF_DESCRIPTION.copied()];
    const PARENT_OPERATION: crate::operations::OperationSpec<Parent> =
        crate::operations::OperationSpec::<Parent>::new(&PARENT_DESCRIPTION).calls(&PARENT_CALLEES);

    fn parent_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            PARENT_OPERATION,
            const { &[] },
            async move |request: Parent, ctx: crate::operations::FunctionContext<Parent>| -> Result<Response<u64>, InvocationError> {
                ctx.operation(LEAF_OPERATION, Leaf { value: request.value }).await
            },
            |_| crate::operations::FailureKind::Permanent,
        )
    }

    struct RetrySettings {
        policy: crate::RetryPolicy,
    }

    #[derive(crate::Message, Clone, Debug, Default)]
    struct Trace {
        value: u64,
    }
    impl crate::operations::ContextValue for Trace {
        const KEY: &'static str = "test.invocation.trace";
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct RetryChild {
        value: u64,
    }

    static RETRY_CHILD_DESCRIPTION: crate::operations::WireDeclaration =
        crate::operations::WireDeclaration::request::<RetryChild>("test.retry_child", POLICY)
            .replay(crate::operations::ReplaySafety::Idempotent);

    const RETRY_CHILD_OPERATION: crate::operations::OperationSpec<RetryChild> =
        crate::operations::OperationSpec::<RetryChild>::new(&RETRY_CHILD_DESCRIPTION);
    async fn retry_child_handler(
        request: RetryChild, ctx: crate::operations::FunctionContext<RetryChild>,
    ) -> Result<Response<u64>, std::io::Error> {
        let trace = ctx.call().value::<Trace>().unwrap().expect("child retains portable metadata");
        assert_eq!(trace.value, request.value);
        if ctx.resource::<super::HostContext>().expect("declared resource is registered").borrow().attempt() == 0 {
            return Err(std::io::Error::other("transient child failure"));
        }
        Ok(request.value.into())
    }

    fn retry_child_classification(_: &std::io::Error) -> crate::operations::FailureKind {
        crate::operations::FailureKind::Transient
    }

    fn retry_child_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            RETRY_CHILD_OPERATION,
            const { &[crate::Resource::of::<super::HostContext>()] },
            retry_child_handler,
            retry_child_classification,
        )
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct RetryParent {
        value: u64,
        override_retry: bool,
    }

    static RETRY_PARENT_DESCRIPTION: crate::operations::WireDeclaration =
        crate::operations::WireDeclaration::request::<RetryParent>("test.retry_parent", POLICY);

    static RETRY_PARENT_CALLEES: [crate::operations::WireDeclaration; 1] = [RETRY_CHILD_DESCRIPTION.copied()];
    const RETRY_PARENT_OPERATION: crate::operations::OperationSpec<RetryParent> =
        crate::operations::OperationSpec::<RetryParent>::new(&RETRY_PARENT_DESCRIPTION).calls(&RETRY_PARENT_CALLEES);
    async fn retry_parent_handler(
        request: RetryParent, ctx: crate::operations::FunctionContext<RetryParent>,
    ) -> Result<Response<u64>, crate::Error> {
        let invocation = ctx.operation(RETRY_CHILD_OPERATION, RetryChild { value: request.value });
        let response = if request.override_retry {
            invocation.retry(ctx.resource::<RetrySettings>().expect("declared resource is registered").policy).await?
        } else {
            invocation.await?
        };
        Ok(response)
    }

    fn retry_parent_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            RETRY_PARENT_OPERATION,
            const { &[crate::Resource::of::<RetrySettings>()] },
            retry_parent_handler,
            |_| crate::operations::FailureKind::Permanent,
        )
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Uncertain {}

    static UNCERTAIN_DESCRIPTION: crate::operations::WireDeclaration = crate::operations::WireDeclaration::request::<Uncertain>(
        "test.uncertain",
        crate::OperationPolicy {
            retries: crate::RetryPolicy::OnTransient {
                total_attempts: 2,
                backoff: crate::Backoff::Fixed {
                    delay: std::time::Duration::from_millis(1),
                    jitter: crate::Jitter::None,
                },
            },
            ..POLICY
        },
    );

    const UNCERTAIN_OPERATION: crate::operations::OperationSpec<Uncertain> =
        crate::operations::OperationSpec::<Uncertain>::new(&UNCERTAIN_DESCRIPTION);
    async fn uncertain_handler(
        _request: Uncertain, ctx: crate::operations::FunctionContext<Uncertain>,
    ) -> Result<Response<u64>, std::io::Error> {
        if ctx.resource::<super::HostContext>().expect("declared resource is registered").borrow().attempt() == 0 {
            return Err(std::io::Error::other("transient"));
        }
        Ok(7.into())
    }

    fn uncertain_classification(_: &std::io::Error) -> crate::operations::FailureKind {
        crate::operations::FailureKind::Transient
    }

    fn uncertain_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            UNCERTAIN_OPERATION,
            const { &[crate::Resource::of::<super::HostContext>()] },
            uncertain_handler,
            uncertain_classification,
        )
    }

    type Evidence = Arc<Mutex<Vec<(String, usize, crate::operations::OperationOutcome, Option<u64>)>>>;
    struct Recorder(Evidence);
    impl crate::operations::middleware::Middleware for Recorder {
        fn enter<'a>(&'a self, call: &'a mut crate::operations::middleware::Invocation) -> crate::operations::middleware::Enter<'a> {
            Box::pin(async move {
                if call.context().value::<Trace>()?.is_none() {
                    call.context_mut().insert_portable(Trace { value: 42 })?;
                }
                Ok(())
            })
        }
        fn attempt<'a>(
            &'a self, call: &'a crate::operations::middleware::Invocation, observation: crate::operations::middleware::AttemptObservation,
        ) -> crate::operations::middleware::Enter<'a> {
            Box::pin(async move {
                let trace = call.context().value::<Trace>().unwrap().map(|trace| trace.value);
                self.0.lock().unwrap().push((call.operation().id.to_string(), observation.attempt, observation.outcome, trace));
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn handler_retry_preserves_defaults_metadata_and_attempt_evidence() {
        let retry = crate::RetryPolicy::OnTransient {
            total_attempts: 2,
            backoff: crate::Backoff::Fixed {
                delay: std::time::Duration::from_millis(1),
                jitter: crate::Jitter::None,
            },
        };
        let evidence = Evidence::default();
        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.resource(RetrySettings { policy: retry });
            setup.settings().operation_middleware(Arc::new(Recorder(evidence.clone())));
        })
        .await
        .unwrap();
        let batch = harmos_operations::execution::__private::Catalog::new()
            .entry(retry_parent_operation().expect("representable operation"))
            .entry(retry_child_operation().expect("representable operation"))
            .entry(uncertain_operation().expect("representable operation"));
        runtime.register_batch(batch.lane(crate::operations::Lane::default())).await.unwrap();
        assert_eq!(*runtime.operation(RetryParent { value: 42, override_retry: true }).await.unwrap(), 42);
        assert!(runtime.operation(RetryParent { value: 42, override_retry: false }).await.is_err());
        // Unknown Replay Safety Refuses Both Declared And Call-Local Retries Before Execution.
        let before = evidence.lock().unwrap().len();
        assert!(matches!(runtime.operation(Uncertain {}).await, Err(crate::Error::InvalidInput { field: "operation retry", .. })));
        assert_eq!(evidence.lock().unwrap().len(), before);
        assert!(runtime.operation(Uncertain {}).retry(crate::RetryPolicy::Never).await.is_err());
        assert!(matches!(runtime.operation(Uncertain {}).await, Err(crate::Error::InvalidInput { field: "operation retry", .. })));
        let before = evidence.lock().unwrap().len();
        assert!(matches!(
            runtime.operation(Uncertain {}).retry(retry).await,
            Err(crate::Error::InvalidInput { field: "operation retry", .. })
        ));
        assert_eq!(evidence.lock().unwrap().len(), before);
        let child =
            evidence.lock().unwrap().iter().filter(|entry| entry.0 == RETRY_CHILD_OPERATION.entry().id).cloned().collect::<Vec<_>>();
        assert_eq!(
            child.iter().map(|entry| (entry.1, entry.2, entry.3)).collect::<Vec<_>>(),
            vec![
                (0, crate::operations::OperationOutcome::Failed, Some(42)),
                (1, crate::operations::OperationOutcome::Succeeded, Some(42)),
                (0, crate::operations::OperationOutcome::Failed, Some(42)),
            ]
        );
        runtime.stop().await.unwrap();
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = ())]
    #[derive(Clone)]
    struct Deadline {}

    static DEADLINE_DESCRIPTION: crate::operations::WireDeclaration = crate::operations::WireDeclaration::request::<Deadline>(
        "test.deadline",
        crate::OperationPolicy {
            attempt_timeout: std::time::Duration::from_millis(10),
            ..POLICY
        },
    )
    .replay(crate::operations::ReplaySafety::Idempotent);

    const DEADLINE_OPERATION: crate::operations::OperationSpec<Deadline> =
        crate::operations::OperationSpec::<Deadline>::new(&DEADLINE_DESCRIPTION);

    fn deadline_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            DEADLINE_OPERATION,
            const { &[] },
            async move |_request: Deadline,
                        _: crate::operations::FunctionContext<Deadline>|
                        -> Result<Response<()>, std::convert::Infallible> { std::future::pending().await },
            |_| crate::operations::FailureKind::Permanent,
        )
    }

    #[tokio::test(start_paused = true)]
    async fn retry_override_keeps_declared_attempt_deadline() {
        let evidence = Evidence::default();
        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.settings().operation_middleware(Arc::new(Recorder(evidence.clone())));
        })
        .await
        .unwrap();
        runtime
            .register_batch(
                harmos_operations::execution::__private::Catalog::new()
                    .entry(deadline_operation().expect("representable operation"))
                    .lane(crate::operations::Lane::default()),
            )
            .await
            .unwrap();
        let retry = crate::RetryPolicy::OnTransient {
            total_attempts: 2,
            backoff: crate::Backoff::Fixed {
                delay: std::time::Duration::from_millis(1),
                jitter: crate::Jitter::None,
            },
        };
        let error = runtime.operation(Deadline {}).retry(retry).await.unwrap_err();
        assert!(
            matches!(error, crate::Error::OperationRefused(crate::Refusal::TimedOut { ref operation, attempt: 2, timeout, configured_by: "operation policy.attempt_timeout" }) if operation == "test.deadline" && timeout == std::time::Duration::from_millis(10))
        );
        let observed = evidence.lock().unwrap().iter().map(|entry| (entry.1, entry.2)).collect::<Vec<_>>();
        assert_eq!(observed, [(0, crate::operations::OperationOutcome::TimedOut), (1, crate::operations::OperationOutcome::TimedOut)]);
        assert!(runtime.operation(Deadline {}).await.is_err());
        assert_eq!(evidence.lock().unwrap().len(), 3);
        runtime.stop().await.unwrap();
    }

    struct TimingProbe {
        now: Mutex<crate::operations::middleware::AttemptStartedAt>,
        observed: Mutex<Vec<crate::operations::middleware::AttemptObservation>>,
    }

    impl TimingProbe {
        fn advance(&self, elapsed: std::time::Duration) {
            self.now.lock().unwrap().instant += elapsed;
        }
    }

    struct TimingRecorder(Arc<TimingProbe>);

    impl crate::operations::middleware::Middleware for TimingRecorder {
        fn enter<'a>(&'a self, _: &'a mut crate::operations::middleware::Invocation) -> crate::operations::middleware::Enter<'a> {
            Box::pin(async move {
                self.0.advance(std::time::Duration::from_secs(3));
                Ok(())
            })
        }

        fn attempt<'a>(
            &'a self, _: &'a crate::operations::middleware::Invocation, observation: crate::operations::middleware::AttemptObservation,
        ) -> crate::operations::middleware::Enter<'a> {
            Box::pin(async move {
                self.0.observed.lock().unwrap().push(observation);
                self.0.advance(std::time::Duration::from_secs(5));
                Ok(())
            })
        }
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = ())]
    #[derive(Clone)]
    struct TimedAttempt {
        refuse: bool,
        timeout: bool,
    }

    static TIMED_ATTEMPT_DESCRIPTION: crate::operations::WireDeclaration = crate::operations::WireDeclaration::request::<TimedAttempt>(
        "test.timed_attempt",
        crate::OperationPolicy {
            attempt_timeout: std::time::Duration::from_millis(10),
            ..POLICY
        },
    )
    .replay(crate::operations::ReplaySafety::Idempotent);

    const TIMED_ATTEMPT_OPERATION: crate::operations::OperationSpec<TimedAttempt> =
        crate::operations::OperationSpec::<TimedAttempt>::new(&TIMED_ATTEMPT_DESCRIPTION);

    async fn timed_attempt_handler(
        request: TimedAttempt, ctx: crate::operations::FunctionContext<TimedAttempt>,
    ) -> Result<Response<()>, std::io::Error> {
        let attempt = ctx.resource::<super::HostContext>().expect("declared resource is registered").borrow().attempt();
        ctx.resource::<TimingProbe>()
            .expect("declared resource is registered")
            .advance(std::time::Duration::from_millis(17 + attempt as u64));
        if request.timeout {
            return std::future::pending().await;
        }
        if request.refuse && attempt == 0 {
            return Err(std::io::Error::other("refused"));
        }
        Ok(().into())
    }

    fn timed_attempt_classification(_: &std::io::Error) -> crate::operations::FailureKind {
        crate::operations::FailureKind::Transient
    }

    fn timed_attempt_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            TIMED_ATTEMPT_OPERATION,
            const { &[crate::Resource::of::<TimingProbe>(), crate::Resource::of::<super::HostContext>()] },
            timed_attempt_handler,
            timed_attempt_classification,
        )
    }

    async fn timed_attempt(refuse: bool, timeout: bool, retry: bool) {
        use crate::operations::middleware::{AttemptClock, AttemptStartedAt};
        use std::time::Duration;
        let initial = AttemptStartedAt {
            instant: crate::supervision::executor::now(),
            timestamp: crate::state::Timestamp::UNIX_EPOCH + Duration::from_secs(123),
        };
        let probe = Arc::new(TimingProbe {
            now: Mutex::new(initial),
            observed: Mutex::new(Vec::new()),
        });
        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.resource::<TimingProbe>(probe.clone());
            setup.settings().operation_middleware(Arc::new(TimingRecorder(probe.clone())));
        })
        .await
        .unwrap();
        let clock = probe.clone();
        runtime.operations.set_attempt_clock(AttemptClock::new(move || *clock.now.lock().unwrap())).unwrap();
        runtime
            .register_batch(
                harmos_operations::execution::__private::Catalog::new()
                    .entry(timed_attempt_operation().expect("representable operation"))
                    .lane(crate::operations::Lane::default()),
            )
            .await
            .unwrap();
        let policy = if retry {
            crate::RetryPolicy::OnTransient {
                total_attempts: 2,
                backoff: crate::Backoff::Fixed {
                    delay: Duration::from_millis(1),
                    jitter: crate::Jitter::None,
                },
            }
        } else {
            crate::RetryPolicy::Never
        };
        let result = runtime.operation(TimedAttempt { refuse, timeout }).retry(policy).await;
        assert_eq!(result.is_ok(), !timeout && (!refuse || retry));
        {
            let observations = probe.observed.lock().unwrap();
            assert_eq!(observations.len(), if retry { 2 } else { 1 });
            for (attempt, observation) in observations.iter().enumerate() {
                let outcome = if timeout {
                    crate::operations::OperationOutcome::TimedOut
                } else if refuse && attempt == 0 {
                    crate::operations::OperationOutcome::Failed
                } else {
                    crate::operations::OperationOutcome::Succeeded
                };
                assert_eq!(observation.attempt, attempt);
                assert_eq!(observation.outcome, outcome);
                assert_eq!(observation.elapsed, Duration::from_millis(17 + attempt as u64));
                assert_eq!(observation.started_at.timestamp, initial.timestamp);
                let prior = if attempt == 0 { Duration::ZERO } else { Duration::from_secs(5) + Duration::from_millis(17) };
                assert_eq!(observation.started_at.instant, initial.instant + Duration::from_secs(3) + prior);
            }
        }
        runtime.stop().await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_timing_success_excludes_middleware() {
        timed_attempt(false, false, false).await;
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_timing_refusal() {
        timed_attempt(true, false, false).await;
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_timing_timeout() {
        timed_attempt(false, true, false).await;
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_timing_retry_is_per_handler() {
        timed_attempt(true, false, true).await;
    }

    #[cfg(feature = "grpc")]
    mod deadline_overrides {
        use super::*;

        #[tokio::test(start_paused = true)]
        async fn inherited_deadline_reports_applied_override() {
            let runtime = crate::Runtime::<Model>::start((), ()).await.unwrap();
            runtime
                .register_batch(
                    harmos_operations::execution::__private::Catalog::new()
                        .entry(deadline_operation().expect("representable operation"))
                        .lane(crate::operations::Lane::default()),
                )
                .await
                .unwrap();
            let timeout = std::time::Duration::from_millis(5);
            let mut context = crate::operations::CallContext::default();
            context.insert(crate::operations::context::Deadline(crate::supervision::executor::now() + timeout)).unwrap();
            let operations = runtime.operations.with_context(context);
            let error = operations.operation::<Deadline>(Deadline {}).await.unwrap_err();
            assert!(matches!(error, crate::Error::OperationRefused(crate::Refusal::TimedOut {
                ref operation, attempt: 1, timeout: applied, configured_by: "per-call gRPC deadline override",
            }) if operation == "test.deadline" && applied == timeout));
            runtime.stop().await.unwrap();
        }
    }

    struct CancellationProbe {
        started: tokio::sync::Notify,
        cancelled: std::sync::atomic::AtomicBool,
    }

    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = ())]
    #[derive(Clone)]
    struct WaitChild {}

    static WAIT_CHILD_DESCRIPTION: crate::operations::WireDeclaration = crate::operations::WireDeclaration::request::<WaitChild>(
        "test.wait_child",
        crate::OperationPolicy {
            cancellation_timeout: std::time::Duration::from_millis(100),
            ..POLICY
        },
    );

    const WAIT_CHILD_OPERATION: crate::operations::OperationSpec<WaitChild> =
        crate::operations::OperationSpec::<WaitChild>::new(&WAIT_CHILD_DESCRIPTION);
    async fn wait_child_handler(
        _request: WaitChild, ctx: crate::operations::FunctionContext<WaitChild>,
    ) -> Result<Response<()>, std::convert::Infallible> {
        let probe = ctx.resource::<CancellationProbe>().expect("declared resource is registered");
        probe.started.notify_one();
        ctx.cancelled().await;
        probe.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(().into())
    }

    fn wait_child_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            WAIT_CHILD_OPERATION,
            const { &[crate::Resource::of::<CancellationProbe>()] },
            wait_child_handler,
            |_| crate::operations::FailureKind::Permanent,
        )
    }
    #[derive(crate::Message, Debug, Default)]
    #[harmos_path(crate)]
    #[derive(crate::Request)]
    #[request(response = ())]
    #[derive(Clone)]
    struct WaitParent {}

    static WAIT_PARENT_DESCRIPTION: crate::operations::WireDeclaration =
        crate::operations::WireDeclaration::request::<WaitParent>("test.wait_parent", POLICY);

    static WAIT_PARENT_CALLEES: [crate::operations::WireDeclaration; 1] = [WAIT_CHILD_DESCRIPTION.copied()];
    const WAIT_PARENT_OPERATION: crate::operations::OperationSpec<WaitParent> =
        crate::operations::OperationSpec::<WaitParent>::new(&WAIT_PARENT_DESCRIPTION).calls(&WAIT_PARENT_CALLEES);

    fn wait_parent_operation() -> ::core::result::Result<crate::operations::OperationFactory, crate::operations::ServingError> {
        crate::operations::OperationFactory::function(
            WAIT_PARENT_OPERATION,
            const { &[] },
            async move |_request: WaitParent,
                        ctx: crate::operations::FunctionContext<WaitParent>|
                        -> Result<Response<()>, InvocationError> { ctx.operation(WAIT_CHILD_OPERATION, WaitChild {}).await },
            |_| crate::operations::FailureKind::Permanent,
        )
    }

    #[tokio::test]
    async fn handler_cancellation_settles_the_owned_child() {
        let probe = Arc::new(CancellationProbe {
            started: tokio::sync::Notify::new(),
            cancelled: std::sync::atomic::AtomicBool::new(false),
        });
        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.resource::<CancellationProbe>(probe.clone());
        })
        .await
        .unwrap();
        let batch = harmos_operations::execution::__private::Catalog::new()
            .entry(wait_parent_operation().expect("representable operation"))
            .entry(wait_child_operation().expect("representable operation"));
        runtime.register_batch(batch.lane(crate::operations::Lane::default())).await.unwrap();
        let running = runtime.operation(WaitParent {}).start().await.unwrap();
        probe.started.notified().await;
        assert!(
            matches!(running.cancel().await, Err(crate::Error::OperationRefused(crate::Refusal::Cancelled { operation, attempt: 1 })) if operation == "test.wait_parent")
        );
        assert!(probe.cancelled.load(std::sync::atomic::Ordering::SeqCst));
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn handler_child_retains_admission_ancestors() {
        let retry = crate::RetryPolicy::Never;
        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.resource(RetrySettings { policy: retry });
        })
        .await
        .unwrap();
        let batch = harmos_operations::execution::__private::Catalog::new()
            .entry(retry_parent_operation().expect("representable operation"))
            .entry(retry_child_operation().expect("representable operation"));
        runtime.register_batch(batch.lane(crate::operations::Lane::new("Serial", crate::operations::Capacity::serial()))).await.unwrap();
        let error = runtime.operation(RetryParent { value: 42, override_retry: false }).await.unwrap_err();
        let crate::Error::OperationFailed { source, .. } = error else {
            panic!("parent must preserve the child failure");
        };
        let Some(crate::Error::OperationInvocation(InvocationError::Failed { source, .. })) = source.downcast_ref::<crate::Error>() else {
            panic!("child must retain the admission cause");
        };
        assert!(matches!(source.downcast_ref::<crate::Error>(), Some(crate::Error::OperationLaneCycle { .. })));
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn declared_calls_verify_activation_and_run_with_resources() {
        let runtime = crate::Runtime::<Model>::start((), ()).await.unwrap();
        let missing = harmos_operations::execution::__private::Catalog::new().entry(parent_operation().expect("representable operation"));
        assert!(runtime.register_batch(missing.lane(crate::operations::Lane::default())).await.is_err());
        let missing = harmos_operations::execution::__private::Catalog::new().entry(leaf_operation().expect("representable operation"));
        assert!(runtime.register_batch(missing.lane(crate::operations::Lane::default())).await.is_err());
        runtime.stop().await.unwrap();
        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.resource(7_u64);
        })
        .await
        .unwrap();
        let batch = harmos_operations::execution::__private::Catalog::new()
            .entry(parent_operation().expect("representable operation"))
            .entry(leaf_operation().expect("representable operation"));
        runtime.register_batch(batch.lane(crate::operations::Lane::default())).await.unwrap();
        let direct = runtime.operation(Leaf { value: 5 }).await.unwrap();
        let child = runtime.operation(Parent { value: 5 }).await.unwrap();
        assert_eq!(*child, 12);
        assert_eq!(*direct, *child);
        runtime.stop().await.unwrap();
    }

    #[tokio::test]
    async fn typed_calls_keep_carrier_and_runtime_resources() {
        struct Carrier(u32);

        impl harmos_operations::execution::operations::Carrier for Carrier {
            type Declaration = Leaf;
            type Error = harmos_operations::execution::ResourceError;
            type StreamEndpoints = ();

            async fn bind(context: harmos_operations::execution::ExecutionContext) -> Result<Self, Self::Error> {
                let handle = context.resource::<u32>()?;
                Ok(Self(*handle))
            }

            async fn attempt(
                &mut self, context: harmos_operations::execution::ExecutionContext, request: &mut Leaf, _: &mut (),
            ) -> Result<Response<u64>, Self::Error> {
                let resource = context.resource::<u64>()?;
                let response = request.value + u64::from(self.0) + *resource;
                Ok(response.into())
            }
        }

        let runtime = crate::Runtime::<Model>::start_with((), (), |setup| {
            setup.resource(7_u64);
        })
        .await
        .unwrap();

        let mut batch = harmos_operations::execution::__private::Catalog::new()
            .carrier::<Leaf, Carrier>(LEAF_OPERATION)
            .expect("representable carrier");
        let factory = batch.operations.pop().unwrap();
        let binding = Arc::new(factory);
        let lane = runtime.operations.instances.lane(crate::operations::Lane::default().name()).unwrap();
        let definition = LEAF_OPERATION.definition().expect("valid operation");
        let mut entry = runtime
            .operations
            .binding_erased(
                "host",
                lane,
                binding,
                definition,
                None,
                #[cfg(feature = "plugin-guest")]
                None,
            )
            .unwrap();

        let handle = Arc::new(11_u32);
        let staged = Arc::get_mut(&mut entry).unwrap();
        staged.carrier.insert(handle).unwrap();
        runtime.operations.reserve(&[entry]).unwrap();

        let response = runtime.operation(Leaf { value: 5 }).await.unwrap();
        assert_eq!(*response, 23);
        let resources = runtime.operations.resources();
        assert!(resources.resource::<u32>().is_err());
        let owner = resources.resource::<u64>().unwrap();
        assert_eq!(*owner, 7);
        runtime.stop().await.unwrap();
    }
}
