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
use super::HostContext;
use harmos_domain::{MemorySize, Message, MessageError, OperationDeclaration};
use harmos_sdk::__private::ErasedResponse;
use harmos_sdk::operations::{CallFuture, Caller, ChildInstance, ControlFuture, InvocationError, StartFuture, Target};

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

struct Request {
    payload: Mutex<Option<Box<dyn Any + Send>>>,
    memory: usize,
}

struct Answer(ErasedResponse);

struct NativeCall;

struct NativeChild(super::Running<NativeCall>, &'static str);

struct EncodedChild {
    invocation: Option<super::EncodedInvocation>,
    target: Target,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Caller for HostContext {
    fn start(
        &self, target: Target, request: Box<dyn Any + Send>, retry: Option<super::RetryPolicy>, endpoints: Box<dyn Any + Send>,
        call: super::CallContext,
    ) -> StartFuture<'_> {
        Box::pin(async move {
            let result = async {
                let mut context = self.borrow();
                context.operations = context.operations.with_context(call);
                let origin = context.source.check()?;
                let entry = context.operations.entry_as(target.id, &origin)?;
                if entry.declared().fingerprint != (target.definition)().fingerprint {
                    return Err(crate::Error::InvalidInput {
                        field: "declared operation",
                        reason: "callee fingerprint differs",
                    });
                }
                let Some(factory) = entry.binding.downcast_ref::<harmos_sdk::operations::OperationFactory>().cloned() else {
                    let payload = (target.encode)(request.as_ref()).ok_or(crate::Error::InvalidInput {
                        field: "declared operation",
                        reason: "callee payload differs",
                    })?;
                    if !endpoints.is::<()>() {
                        return Err(crate::Error::InvalidInput {
                            field: "operation endpoints",
                            reason: "encoded children do not accept native endpoints",
                        });
                    }
                    let invocation = context.submit_encoded_target(entry, payload, retry)?;
                    return Ok(Box::new(EncodedChild { invocation: Some(invocation), target }) as Box<dyn ChildInstance>);
                };
                if !factory.accepts_target(&target, endpoints.as_ref().type_id()) {
                    return Err(crate::Error::InvalidInput {
                        field: "declared operation",
                        reason: "callee payload types differ",
                    });
                }
                let running = if endpoints.is::<()>() {
                    admit_native(&context, target, request, retry, (), factory, |endpoints| Box::new(endpoints))?
                } else {
                    admit_native(&context, target, request, retry, endpoints, factory, |endpoints| endpoints)?
                };
                Ok(Box::new(NativeChild(running, target.id)) as Box<dyn ChildInstance>)
            }
            .await;
            result.map_err(|error: crate::Error| InvocationError::failed(target.id, error))
        })
    }
}

impl ChildInstance for NativeChild {
    fn id(&self) -> super::OperationInstanceId {
        self.0.id()
    }
    fn status(&self) -> ControlFuture<'_, harmos_sdk::operations::instance::Status> {
        Box::pin(std::future::ready(Ok(self.0.control.status().portable())))
    }
    fn request_cancel(&self) -> ControlFuture<'_, ()> {
        self.0.request_cancel();
        Box::pin(async { Ok(()) })
    }
    fn join(self: Box<Self>) -> CallFuture<'static> {
        Box::pin(async move { self.0.await.map(|response| response.0.clone()).map_err(|error| InvocationError::failed(self.1, error)) })
    }
    fn detach(self: Box<Self>) -> super::OperationInstanceId {
        self.0.detach()
    }
}

impl ChildInstance for EncodedChild {
    fn id(&self) -> super::OperationInstanceId {
        self.invocation.as_ref().expect("owned child").id()
    }
    fn status(&self) -> ControlFuture<'_, harmos_sdk::operations::instance::Status> {
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
            let answer = invocation.await;
            cancel.0.take();
            let bytes = answer.map_err(|error| InvocationError::failed(self.target.id, error))?;
            (self.target.decode)(&bytes).map_err(|error| InvocationError::failed(self.target.id, error))
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

impl OperationDeclaration for NativeCall {
    type Request = Request;
    type Response = Answer;
    type Input = harmos_domain::operations::NoStream;
    type Output = harmos_domain::operations::NoStream;
    const ID: &'static str = "native.invocation";
    const POLICY: harmos_domain::OperationPolicy = harmos_operations::host::POLICY;
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

/* === Private Functions === */

fn admit_native<P: Send + 'static>(
    context: &super::Context<'_>, target: Target, request: Box<dyn Any + Send>, retry: Option<super::RetryPolicy>, endpoints: P,
    factory: harmos_sdk::operations::OperationFactory, erase: fn(P) -> Box<dyn Any + Send>,
) -> crate::Result<super::Running<NativeCall>> {
    let grant = (target.grant)(request.as_ref());
    let key = (target.key)(request.as_ref());
    let memory = (target.memory)(request.as_ref());
    let entry = context.operations.entry_as(target.id, &context.source.check()?)?;
    let resources = context.operations.resources_for(&entry);
    let binding = super::authored::factory_projected::<NativeCall, P>(
        &context.operations,
        move |request, endpoints| {
            let payload =
                request.payload.into_inner().map_err(|_| harmos_sdk::ServingError::Catalog)?.ok_or(harmos_sdk::ServingError::Catalog)?;
            factory.prepare_native(resources.clone(), payload, erase(endpoints))
        },
        |response| Some(harmos_sdk::Response::new(Answer(response))),
        |response| response.0.clone(),
    );
    let mut input = super::binding::Request::from(Request {
        payload: Mutex::new(Some(request)),
        memory,
    });
    input.retry = retry;
    input.binding = Some(Arc::new(binding));
    input.grant = grant;
    input.key = key;
    let running = context.operations.admit_at::<NativeCall, P>(
        target.id,
        (input, endpoints),
        Some(&context.scope),
        context.admission.held(),
        &context.source,
        None,
    )?;
    Ok(running)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{Lane, RegistrationExt};
    use harmos_sdk::{Context, Operation, Response};

    #[crate::lanes]
    enum Lanes {
        #[default]
        Open,

        #[lane(serial)]
        Serial,
    }
    struct Model;
    impl crate::Lifecycle for Model {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = crate::Startup<Self>;

        async fn prepare(_: (), _: &mut crate::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl crate::Application for Model {
        type Lanes = Lanes;
        type State = ();
        type Transaction = crate::state::Never;
        type Record = ();
        type Streams = ();
    }

    const POLICY: crate::OperationPolicy = crate::OperationPolicy {
        timeout: std::time::Duration::from_secs(2),
        attempt_timeout: std::time::Duration::from_secs(2),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: crate::RetryPolicy::Never,
    };

    #[crate::operation(harmos_path = crate, resources = [u64], id = "test.leaf", response = u64, policy = POLICY,
        key = |request: &Leaf| crate::operations::CacheKey::new(request.value.to_le_bytes()).ok())]
    struct Leaf {
        value: u64,
    }
    impl Operation for Leaf {
        type Error = std::convert::Infallible;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<u64>, Self::Error> {
            Ok((self.value + *ctx.resource::<u64>()).into())
        }
    }

    #[crate::operation(harmos_path = crate, id = "test.parent", response = u64, policy = POLICY, operations = [Leaf])]
    struct Parent {
        value: u64,
    }
    impl Operation for Parent {
        type Error = InvocationError;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<u64>, Self::Error> {
            ctx.operation(Leaf { value: self.value }).await
        }
    }

    struct RetrySettings {
        policy: crate::RetryPolicy,
    }

    #[crate::message]
    struct Trace {
        value: u64,
    }
    impl crate::operations::ContextValue for Trace {
        const KEY: &'static str = "test.invocation.trace";
    }

    #[crate::operation(harmos_path = crate, resources = [super::HostContext], id = "test.retry_child", response = u64,
        policy = POLICY, replay = crate::operations::ReplaySafety::Idempotent)]
    struct RetryChild {
        value: u64,
    }
    impl Operation for RetryChild {
        type Error = std::io::Error;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<u64>, Self::Error> {
            let trace = ctx.call().value::<Trace>().unwrap().expect("child retains portable metadata");
            assert_eq!(trace.value, self.value);
            if ctx.resource::<super::HostContext>().borrow().attempt() == 0 {
                return Err(std::io::Error::other("transient child failure"));
            }
            Ok(self.value.into())
        }
        fn classify(_: &Self::Error) -> crate::operations::FailureKind {
            crate::operations::FailureKind::Transient
        }
    }

    #[crate::operation(harmos_path = crate, resources = [RetrySettings], id = "test.retry_parent", response = u64,
        policy = POLICY, operations = [RetryChild])]
    struct RetryParent {
        value: u64,
        override_retry: bool,
    }
    impl Operation for RetryParent {
        type Error = crate::Error;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<u64>, Self::Error> {
            let invocation = ctx.operation(RetryChild { value: self.value });
            let response =
                if self.override_retry { invocation.retry(ctx.resource::<RetrySettings>().policy).await? } else { invocation.await? };
            Ok(response)
        }
    }

    #[crate::operation(harmos_path = crate, resources = [super::HostContext], id = "test.uncertain", response = u64,
        policy = crate::OperationPolicy { retries: crate::RetryPolicy::OnTransient { max_retries: 1, backoff: crate::Backoff::Fixed { delay: std::time::Duration::from_millis(1), jitter: crate::Jitter::None } }, ..POLICY })]
    struct Uncertain {}
    impl Operation for Uncertain {
        type Error = std::io::Error;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<u64>, Self::Error> {
            if ctx.resource::<super::HostContext>().borrow().attempt() == 0 {
                return Err(std::io::Error::other("transient"));
            }
            Ok(7.into())
        }
        fn classify(_: &Self::Error) -> crate::operations::FailureKind {
            crate::operations::FailureKind::Transient
        }
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
            max_retries: 1,
            backoff: crate::Backoff::Fixed {
                delay: std::time::Duration::from_millis(1),
                jitter: crate::Jitter::None,
            },
        };
        let evidence = Evidence::default();
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource(RetrySettings { policy: retry });
            setup.settings().operation_middleware(Arc::new(Recorder(evidence.clone())));
        })
        .await
        .unwrap();
        let batch =
            harmos_sdk::__private::RegistrationBatch::new().operation::<RetryParent>().operation::<RetryChild>().operation::<Uncertain>();
        runtime.register_batch(batch.lane(<Lanes as Lane>::default_lane())).await.unwrap();
        assert_eq!(*runtime.operation(RetryParent { value: 42, override_retry: true }).await.unwrap(), 42);
        assert!(runtime.operation(RetryParent { value: 42, override_retry: false }).await.is_err());
        // Macro Defaults Retain Existing Transient Retry Semantics Even Without Replay Safety.
        assert_eq!(*runtime.operation(Uncertain {}).await.unwrap(), 7);
        assert!(runtime.operation(Uncertain {}).retry(crate::RetryPolicy::Never).await.is_err());
        assert_eq!(*runtime.operation(Uncertain {}).await.unwrap(), 7);
        let before = evidence.lock().unwrap().len();
        assert!(matches!(
            runtime.operation(Uncertain {}).retry(retry).await,
            Err(crate::Error::InvalidInput { field: "operation retry", .. })
        ));
        assert_eq!(evidence.lock().unwrap().len(), before);
        let child = evidence.lock().unwrap().iter().filter(|entry| entry.0 == RetryChild::ID).cloned().collect::<Vec<_>>();
        assert_eq!(
            child.iter().map(|entry| (entry.1, entry.2, entry.3)).collect::<Vec<_>>(),
            vec![
                (0, crate::operations::OperationOutcome::Failed, Some(42)),
                (1, crate::operations::OperationOutcome::Succeeded, Some(42)),
                (0, crate::operations::OperationOutcome::Failed, Some(42)),
            ]
        );
        runtime.shutdown().await.unwrap();
    }

    #[crate::operation(harmos_path = crate, id = "test.deadline", response = (),
        policy = crate::OperationPolicy { attempt_timeout: std::time::Duration::from_millis(10), ..POLICY },
        replay = crate::operations::ReplaySafety::Idempotent)]
    struct Deadline {}
    impl Operation for Deadline {
        type Error = std::convert::Infallible;
        async fn run(self, _: Context<'_, Self>) -> Result<Response<()>, Self::Error> {
            std::future::pending().await
        }
    }

    #[tokio::test(start_paused = true)]
    async fn retry_override_keeps_declared_attempt_deadline() {
        let evidence = Evidence::default();
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.settings().operation_middleware(Arc::new(Recorder(evidence.clone())));
        })
        .await
        .unwrap();
        runtime.register_batch(harmos_sdk::__private::RegistrationBatch::new().operation::<Deadline>().lane(Lanes::Open)).await.unwrap();
        let retry = crate::RetryPolicy::OnTransient {
            max_retries: 1,
            backoff: crate::Backoff::Fixed {
                delay: std::time::Duration::from_millis(1),
                jitter: crate::Jitter::None,
            },
        };
        let error = runtime.operation(Deadline {}).retry(retry).await.unwrap_err();
        assert!(
            matches!(error, crate::Error::OperationRefused(crate::Refusal::TimedOut { ref operation, attempt: 2, timeout, configured_by: "operation policy.attempt_timeout" }) if operation == "test.deadline" && timeout == std::time::Duration::from_millis(10))
        );
        let displayed = error.to_string();
        assert!(displayed.contains("test.deadline") && displayed.contains("0.01 s deadline") && displayed.contains("attempt 2"));
        let observed = evidence.lock().unwrap().iter().map(|entry| (entry.1, entry.2)).collect::<Vec<_>>();
        assert_eq!(observed, [(0, crate::operations::OperationOutcome::TimedOut), (1, crate::operations::OperationOutcome::TimedOut)]);
        assert!(runtime.operation(Deadline {}).await.is_err());
        assert_eq!(evidence.lock().unwrap().len(), 3);
        runtime.shutdown().await.unwrap();
    }

    #[cfg(feature = "grpc")]
    mod deadline_overrides {
        use super::*;

        #[tokio::test(start_paused = true)]
        async fn inherited_deadline_reports_applied_override() {
            let runtime = crate::Runtime::<Model>::launch((), ()).await.unwrap();
            runtime
                .register_batch(harmos_sdk::__private::RegistrationBatch::new().operation::<Deadline>().lane(Lanes::Open))
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
            runtime.shutdown().await.unwrap();
        }
    }

    struct CancellationProbe {
        started: tokio::sync::Notify,
        cancelled: std::sync::atomic::AtomicBool,
    }

    #[crate::operation(harmos_path = crate, resources = [CancellationProbe], id = "test.wait_child", response = (),
        policy = crate::OperationPolicy { cancellation_timeout: std::time::Duration::from_millis(100), ..POLICY })]
    struct WaitChild {}
    impl Operation for WaitChild {
        type Error = std::convert::Infallible;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<()>, Self::Error> {
            let probe = ctx.resource::<CancellationProbe>();
            probe.started.notify_one();
            ctx.cancelled().await;
            probe.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(().into())
        }
    }
    #[crate::operation(harmos_path = crate, id = "test.wait_parent", response = (), policy = POLICY, operations = [WaitChild])]
    struct WaitParent {}
    impl Operation for WaitParent {
        type Error = InvocationError;
        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<()>, Self::Error> {
            ctx.operation(WaitChild {}).await
        }
    }

    #[tokio::test]
    async fn handler_cancellation_settles_the_owned_child() {
        let probe = Arc::new(CancellationProbe {
            started: tokio::sync::Notify::new(),
            cancelled: std::sync::atomic::AtomicBool::new(false),
        });
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource::<CancellationProbe>(probe.clone());
        })
        .await
        .unwrap();
        let batch = harmos_sdk::__private::RegistrationBatch::new().operation::<WaitParent>().operation::<WaitChild>();
        runtime.register_batch(batch.lane(Lanes::Open)).await.unwrap();
        let running = runtime.operation(WaitParent {}).start().await.unwrap();
        probe.started.notified().await;
        assert!(
            matches!(running.cancel().await, Err(crate::Error::OperationRefused(crate::Refusal::Cancelled { operation, attempt: 1 })) if operation == "test.wait_parent")
        );
        assert!(probe.cancelled.load(std::sync::atomic::Ordering::SeqCst));
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn handler_child_retains_admission_ancestors() {
        let retry = crate::RetryPolicy::Never;
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource(RetrySettings { policy: retry });
        })
        .await
        .unwrap();
        let batch = harmos_sdk::__private::RegistrationBatch::new().operation::<RetryParent>().operation::<RetryChild>();
        runtime.register_batch(batch.lane(Lanes::Serial)).await.unwrap();
        let error = runtime.operation(RetryParent { value: 42, override_retry: false }).await.unwrap_err();
        let crate::Error::OperationFailed { source, .. } = error else {
            panic!("parent must preserve the child failure");
        };
        let Some(crate::Error::OperationInvocation(InvocationError::Failed { source, .. })) = source.downcast_ref::<crate::Error>() else {
            panic!("child must retain the admission cause");
        };
        assert!(matches!(source.downcast_ref::<crate::Error>(), Some(crate::Error::OperationLaneCycle { .. })));
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn declared_calls_verify_activation_and_run_with_resources() {
        let runtime = crate::Runtime::<Model>::launch((), ()).await.unwrap();
        let missing = harmos_sdk::__private::RegistrationBatch::new().operation::<Parent>();
        assert!(runtime.register_batch(missing.lane(<Lanes as crate::operations::Lane>::default_lane())).await.is_err());
        let missing = harmos_sdk::__private::RegistrationBatch::new().operation::<Leaf>();
        assert!(runtime.register_batch(missing.lane(<Lanes as crate::operations::Lane>::default_lane())).await.is_err());
        runtime.shutdown().await.unwrap();
        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource(7_u64);
        })
        .await
        .unwrap();
        let batch = harmos_sdk::__private::RegistrationBatch::new().operation::<Parent>().operation::<Leaf>();
        runtime.register_batch(batch.lane(<Lanes as crate::operations::Lane>::default_lane())).await.unwrap();
        runtime.operations.set_cache_capacity(2);
        let direct = runtime.operation(Leaf { value: 5 }).await.unwrap();
        let child = runtime.operation(Parent { value: 5 }).await.unwrap();
        assert_eq!(*child, 12);
        assert!(direct.ptr_eq(&child));
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn typed_calls_keep_carrier_and_runtime_resources() {
        struct Carrier(u32);

        impl harmos_sdk::operations::Carrier for Carrier {
            type Declaration = Leaf;
            type Error = harmos_sdk::ResourceError;
            type StreamEndpoints = ();

            async fn bind(context: harmos_sdk::ExecutionContext) -> Result<Self, Self::Error> {
                let handle = context.resource::<u32>()?;
                Ok(Self(*handle))
            }

            async fn attempt(
                &mut self, context: harmos_sdk::ExecutionContext, request: &mut Leaf, _: &mut (),
            ) -> Result<Response<u64>, Self::Error> {
                let resource = context.resource::<u64>()?;
                let response = request.value + u64::from(self.0) + *resource;
                Ok(response.into())
            }
        }

        let runtime = crate::Runtime::<Model>::launch_with((), (), |setup| {
            setup.resource(7_u64);
        })
        .await
        .unwrap();

        let mut batch = harmos_sdk::__private::RegistrationBatch::new().carrier::<Leaf, Carrier>();
        let factory = batch.operations.pop().unwrap();
        let binding = Arc::new(factory);
        let lane = <Lanes as Lane>::default_lane().name();
        let definition = Leaf::definition();
        let mut entry = runtime
            .operations
            .binding_erased(
                "host",
                lane,
                binding,
                definition,
                None,
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
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
        runtime.shutdown().await.unwrap();
    }
}
