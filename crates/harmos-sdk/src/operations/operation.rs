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
use super::calls::{Calls, InvocationError, Target};
use super::{
    CallContext, EndpointSet, Executable, FailureKind, Handler, Invokes, NoStream, OperationContext, OperationDeclaration, Requires,
    Response,
};

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
    execution: &'a OperationContext,
    streams: &'a O::StreamEndpoints,
}

// The Handler Behind Every Authored Operation; Each Attempt Runs A Clone Of
// The Request So A Retry Keeps The Original.
pub struct Authored<O>(PhantomData<fn() -> O>);

/* === Enums === */

/* === Traits === */

// An Operation Whose Struct Is Its Request Message.
pub trait Operation: OperationDeclaration<Request = Self> + crate::Portable + Streams + Dependent + Clone + Send + 'static {
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
    pub fn operation<T>(&self, request: T) -> impl Future<Output = Result<Response<T::Response>, InvocationError>> + Send + use<'_, O, T>
    where
        O: Invokes<T>,
        T: OperationDeclaration<Request = T> + Send,
    {
        let calls = self.execution.resource::<Calls>().map_err(|_| InvocationError::Unavailable { operation: T::ID });
        async move {
            let response = calls?.0.call(Target::of::<T>(), Box::new(request)).await?;
            response.restore::<T::Response>().ok_or(InvocationError::Response { operation: T::ID })
        }
    }
}

impl<O: Operation> Handler for Authored<O> {
    type Request = O;
    type Response = O::Response;
    type Error = O::Error;
    type StreamEndpoints = O::StreamEndpoints;

    async fn initialize(_: OperationContext) -> Result<Self, O::Error> {
        Ok(Self(PhantomData))
    }

    async fn run(
        &mut self, context: OperationContext, request: &mut O, endpoints: &mut O::StreamEndpoints,
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
    type Handler = Authored<O>;
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
impl<T: crate::Portable> Shape for (Stream<T>, NoStream) {
    type StreamEndpoints = super::StreamInput<T>;
}

#[cfg(feature = "streaming")]
impl<T: crate::Portable> Shape for (NoStream, Stream<T>) {
    type StreamEndpoints = super::StreamOutput<T>;
}

#[cfg(feature = "streaming")]
impl<I: crate::Portable, T: crate::Portable> Shape for (Stream<I>, Stream<T>) {
    type StreamEndpoints = (super::StreamInput<I>, super::StreamOutput<T>);
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::{Dependencies, RegistrationBatch};

    #[crate::message(harmos_path = crate)]
    #[derive(Clone)]
    struct Increment {
        value: u64,
    }

    impl OperationDeclaration for Increment {
        type Request = Self;
        type Response = u64;
        type Input = NoStream;
        type Output = NoStream;
        const ID: &'static str = "test.increment";
        const POLICY: crate::OperationPolicy = crate::OperationPolicy {
            timeout: std::time::Duration::from_secs(1),
            attempt_timeout: std::time::Duration::from_secs(1),
            retries: crate::RetryPolicy::Never,
        };
    }
    impl Requires<u64> for Increment {}
    impl Dependent for Increment {
        const RESOURCES: &'static [Resource] = &[Resource::of::<u64>()];
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
        let missing = RegistrationBatch::new(Dependencies::new()).operation::<Increment>();
        assert!(missing.operations[0].verify_resources(&[]).is_err());
        assert!(missing.operations[0].verify_resources(&[TypeId::of::<u64>()]).is_ok());
        let mut dependencies = Dependencies::new();
        dependencies.insert(Arc::new(2_u64)).unwrap();
        let batch = RegistrationBatch::new(dependencies).operation::<Increment>();
        let factory = &batch.operations[0];
        factory.verify_resources(&[]).unwrap();
        let mut instance = factory.prepare_native(Box::new(Increment { value: 5 }), Box::new(())).unwrap();
        let context = OperationContext::new(Dependencies::new(), CallContext::default());
        ready(instance.initialize(context.clone())).unwrap();
        for _ in 0..2 {
            let answer = ready(instance.run(context.clone())).unwrap().restore::<u64>().unwrap();
            assert_eq!(*answer, 7);
        }
    }
}
