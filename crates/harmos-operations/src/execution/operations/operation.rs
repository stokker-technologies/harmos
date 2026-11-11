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
use std::any::{TypeId, type_name};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

// Third Party Imports

// Local Imports
#[cfg(feature = "streaming")]
use super::Stream;
use super::calls::{Calls, ChildInstance, InvocationError, Target};
use super::{
    CallContext, Carrier, EndpointSet, Executable, ExecutionContext, FailureKind, Invokes, NoStream, OperationDeclaration, Requires,
    Response,
};
use super::{ContextError, ContextValue, Instance, Invocation, OperationHost, RetryPolicy};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// One Execution Resource An Operation Declares With `resources = [..]`.
#[derive(Clone, Copy, Debug)]
pub struct Resource {
    id: fn() -> TypeId,
    name: fn() -> &'static str,
}

// One Attempt Of An Authored Operation: Call Metadata, Declared Resources,
// Typed Stream Endpoints And Declared Child Operations.
pub struct Context<'a, O: Streams> {
    execution: &'a ExecutionContext,
    streams: &'a O::StreamEndpoints,
}

// The Carrier Behind Every Authored Operation; Each Attempt Runs A Clone Of
// The Request So A Retry Keeps The Original.
pub struct Authored<O>(PhantomData<fn() -> O>);

#[derive(Clone)]
pub struct AttemptOperationHost {
    execution: ExecutionContext,
}

pub struct AttemptInstance<O: OperationDeclaration> {
    inner: Box<dyn ChildInstance>,
    operation: PhantomData<fn() -> O>,
}

/* === Enums === */

/* === Traits === */

// An Operation Whose Struct Is Its Request Message.
pub trait Operation:
    OperationDeclaration<Request = Self> + crate::execution::Portable + Streams + Dependent + Clone + Send + 'static
{
    type Error: std::error::Error + Send + Sync + 'static;

    fn run(self, context: Context<'_, Self>) -> impl Future<Output = Result<Response<Self::Response>, Self::Error>> + Send;

    fn classify(_error: &Self::Error) -> FailureKind {
        FailureKind::Permanent
    }
}

// The Resources `#[harmos::operation]` Records For Registration Verification.
pub trait Dependent {
    const RESOURCES: &'static [Resource];
}

// The Endpoint Set Derived From A Declaration's `input` And `output`.
pub trait Streams {
    type StreamEndpoints: EndpointSet;
}

pub trait Shape {
    type StreamEndpoints: EndpointSet;
}

/* === Implementations === */

impl Resource {
    pub const fn of<T: Send + Sync + 'static>() -> Self {
        Self {
            id: TypeId::of::<T>,
            name: type_name::<T>,
        }
    }

    pub fn id(&self) -> TypeId {
        (self.id)()
    }

    pub fn name(&self) -> &'static str {
        (self.name)()
    }
}

impl<O: Streams> Context<'_, O> {
    pub async fn cancelled(&self) {
        self.execution.cancellation.cancelled().await;
    }

    pub fn cancellation_requested(&self) -> bool {
        self.execution.cancellation.is_requested()
    }

    pub fn call(&self) -> &CallContext {
        &self.execution.call
    }

    // Registration Verified Every Declared Resource Before This Attempt Began.
    pub fn resource<T: Send + Sync + 'static>(&self) -> Arc<T>
    where
        O: Requires<T>,
    {
        self.execution.resource::<T>().unwrap_or_else(|error| panic!("declared resource unavailable after verification: {error}"))
    }

    pub fn streams(&self) -> &O::StreamEndpoints {
        self.streams
    }

    // Invoke A Declared Operation With This Attempt's Call Metadata; The Callee's Policy Applies.
    pub fn operation<T>(&self, request: T) -> Invocation<AttemptOperationHost, T>
    where
        O: Invokes<T>,
        T: OperationDeclaration<Request = T> + Send,
    {
        Invocation::new(AttemptOperationHost { execution: self.execution.clone() }, request)
    }
}

impl OperationHost for AttemptOperationHost {
    type Error = InvocationError;
    type Instance<T: OperationDeclaration> = AttemptInstance<T>;

    fn context(&self) -> &CallContext {
        &self.execution.call
    }
    fn with_value<T: ContextValue>(&self, value: T) -> Result<Self, ContextError> {
        let mut child = self.clone();
        child.execution.call = self.execution.call.with_value(value)?;
        Ok(child)
    }

    async fn invoke<T: OperationDeclaration, E: EndpointSet>(
        &self, request: T::Request, retry: Option<RetryPolicy>, endpoints: E,
    ) -> Result<AttemptInstance<T>, InvocationError> {
        let calls = self.execution.resource::<Calls>().map_err(|_| InvocationError::Unavailable { operation: T::ID })?;
        let inner = calls.0.start(Target::of::<T>(), Box::new(request), retry, Box::new(endpoints), self.execution.call.clone()).await?;
        Ok(AttemptInstance { inner, operation: PhantomData })
    }
    async fn submit<T: OperationDeclaration>(&self, request: T::Request) -> Result<AttemptInstance<T>, InvocationError> {
        self.invoke::<T, ()>(request, None, ()).await
    }
    async fn submit_owned<T: OperationDeclaration>(
        &self, _: T::Request, _: super::RequestClaims,
    ) -> Result<AttemptInstance<T>, InvocationError> {
        Err(InvocationError::Options { operation: T::ID })
    }
    async fn publish_on<S: crate::execution::StreamSpec>(&self, _: &S::Row) -> Result<(), InvocationError>
    where
        S::Row: crate::execution::Message,
    {
        Err(InvocationError::Unavailable { operation: S::ID })
    }
}

impl<O: OperationDeclaration> Instance<O> for AttemptInstance<O> {
    type Error = InvocationError;
    fn id(&self) -> super::instance::Id {
        self.inner.id()
    }
    fn status(&self) -> impl Future<Output = Result<super::instance::Status, InvocationError>> + Send {
        self.inner.status()
    }
    fn request_cancel(&self) -> impl Future<Output = Result<(), InvocationError>> + Send {
        self.inner.request_cancel()
    }
    async fn join(self) -> Result<Response<O::Response>, InvocationError> {
        self.inner.join().await?.restore::<O::Response>().ok_or(InvocationError::Response { operation: O::ID })
    }
    fn detach(self) -> super::instance::Id {
        self.inner.detach()
    }
}

impl<O: Operation> Carrier for Authored<O> {
    type Declaration = O;
    type Error = O::Error;
    type StreamEndpoints = O::StreamEndpoints;

    async fn bind(_: ExecutionContext) -> Result<Self, O::Error> {
        Ok(Self(PhantomData))
    }

    async fn attempt(
        &mut self, context: ExecutionContext, request: &mut O, endpoints: &mut O::StreamEndpoints,
    ) -> Result<Response<O::Response>, O::Error> {
        request.clone().run(Context { execution: &context, streams: endpoints }).await
    }

    fn transferred(&self) -> bool {
        false
    }

    fn classify(error: &O::Error) -> FailureKind {
        O::classify(error)
    }
}

impl<O: Operation> Executable for O {
    type Carrier = Authored<O>;
    const RESOURCES: &'static [Resource] = <O as Dependent>::RESOURCES;
}

impl<O: OperationDeclaration> Streams for O
where
    (O::Input, O::Output): Shape,
{
    type StreamEndpoints = <(O::Input, O::Output) as Shape>::StreamEndpoints;
}

impl Shape for (NoStream, NoStream) {
    type StreamEndpoints = ();
}

#[cfg(feature = "streaming")]
impl<T: crate::execution::Portable> Shape for (Stream<T>, NoStream) {
    type StreamEndpoints = super::StreamInput<T>;
}

#[cfg(feature = "streaming")]
impl<T: crate::execution::Portable> Shape for (NoStream, Stream<T>) {
    type StreamEndpoints = super::StreamOutput<T>;
}

#[cfg(feature = "streaming")]
impl<I: crate::execution::Portable, T: crate::execution::Portable> Shape for (Stream<I>, Stream<T>) {
    type StreamEndpoints = (super::StreamInput<I>, super::StreamOutput<T>);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::operations::{RegistrationBatch, ResourceSet};

    #[crate::execution::operation(harmos_path = crate::execution, resources = [u64], id = "test.increment", response = u64,
        policy = crate::execution::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            cancellation_timeout: std::time::Duration::ZERO,
            retries: crate::execution::RetryPolicy::Never,
        },
    )]
    struct Increment {
        value: u64,
    }

    impl Operation for Increment {
        type Error = std::convert::Infallible;
        async fn run(mut self, ctx: Context<'_, Self>) -> Result<Response<u64>, Self::Error> {
            self.value += *ctx.resource::<u64>();
            Ok(self.value.into())
        }
    }

    fn ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("synchronous operation pending"),
        }
    }

    #[test]
    fn declared_resources_and_attempt_clones() {
        let missing = RegistrationBatch::new().operation::<Increment>();
        assert!(missing.operations[0].verify_resources(&ResourceSet::new(), &[]).is_err());
        assert!(missing.operations[0].verify_resources(&ResourceSet::new(), &[TypeId::of::<u64>()]).is_ok());
        let mut dependencies = ResourceSet::new();
        dependencies.insert(Arc::new(2_u64)).unwrap();
        let batch = RegistrationBatch::new().operation::<Increment>();
        let factory = &batch.operations[0];
        factory.verify_resources(&dependencies, &[]).unwrap();
        let mut instance = factory.prepare_native(dependencies, Box::new(Increment { value: 5 }), Box::new(())).unwrap();
        let context = ExecutionContext::new(ResourceSet::new(), CallContext::default());
        ready(instance.initialize(context.clone())).unwrap();
        for _ in 0..2 {
            let answer = ready(instance.run(context.clone())).unwrap().restore::<u64>().unwrap();
            assert_eq!(*answer, 7);
        }
    }
}
