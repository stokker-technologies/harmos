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

// Third Party Imports

// Local Imports
#[cfg(feature = "streaming")]
use super::Stream;
use super::{EndpointSet, NoStream, RequestShape};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug)]
pub struct Resource {
    id: fn() -> TypeId,
    name: fn() -> &'static str,
}

/* === Enums === */

/* === Traits === */

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

impl<Q: RequestShape> Streams for Q
where
    (Q::Input, Q::Output): Shape,
{
    type StreamEndpoints = <(Q::Input, Q::Output) as Shape>::StreamEndpoints;
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
    use crate::execution::__private::{Catalog, ResourceSet};
    use crate::execution::operations::{CallContext, ExecutionContext, Response};
    use std::{future::Future, sync::Arc};

    #[crate::execution::message(harmos_path = crate::execution)]
    #[derive(crate::execution::Request)]
    #[harmos_path(crate::execution)]
    #[request(response = u64)]
    #[derive(Clone)]
    struct Increment {
        value: u64,
    }

    static INCREMENT_DESCRIPTION: crate::execution::operations::WireDeclaration =
        crate::execution::operations::WireDeclaration::request::<Increment>(
            "test.increment",
            crate::execution::OperationPolicy {
                timeout: std::time::Duration::from_secs(1),
                attempt_timeout: std::time::Duration::from_secs(1),
                cancellation_timeout: std::time::Duration::ZERO,
                retries: crate::execution::RetryPolicy::Never,
            },
        );

    const INCREMENT_OPERATION: crate::execution::operations::OperationSpec<Increment> =
        crate::execution::operations::OperationSpec::<Increment>::new(&INCREMENT_DESCRIPTION);

    async fn increment_handler(
        mut request: Increment, ctx: crate::execution::operations::FunctionContext<Increment>,
    ) -> Result<Response<u64>, std::convert::Infallible> {
        request.value += *ctx.resource::<u64>().expect("declared resource is registered");
        Ok(request.value.into())
    }

    fn increment_operation()
    -> ::core::result::Result<crate::execution::operations::OperationFactory, crate::execution::operations::ServingError> {
        crate::execution::operations::OperationFactory::function(
            INCREMENT_OPERATION,
            const { &[crate::execution::Resource::of::<u64>()] },
            increment_handler,
            |_| crate::execution::operations::FailureKind::Permanent,
        )
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
        let missing = Catalog::new().entry(increment_operation().unwrap());
        assert!(missing.operations[0].verify_resources(&ResourceSet::new(), &[]).is_err());
        assert!(missing.operations[0].verify_resources(&ResourceSet::new(), &[TypeId::of::<u64>()]).is_ok());
        let mut dependencies = ResourceSet::new();
        dependencies.insert(Arc::new(2_u64)).unwrap();
        let batch = Catalog::new().entry(increment_operation().unwrap());
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
    #[test]
    fn undeclared_access_fails_with_registered_operation_identity() {
        use crate::execution::operations::{FunctionContext, InvocationError, OperationFactory};
        async fn resource(_: Increment, ctx: FunctionContext<Increment>) -> Result<Response<u64>, InvocationError> {
            Ok((*ctx.resource::<u64>()?).into())
        }
        async fn call(request: Increment, ctx: FunctionContext<Increment>) -> Result<Response<u64>, InvocationError> {
            ctx.operation(INCREMENT_OPERATION, request).await
        }
        let mut dependencies = ResourceSet::new();
        dependencies.insert(Arc::new(9_u64)).unwrap();
        let context = ExecutionContext::new(ResourceSet::new(), CallContext::default());
        let factory = OperationFactory::function(INCREMENT_OPERATION, &[], resource, |_| super::super::FailureKind::Permanent)
            .unwrap()
            .with_id("test.renamed_resource")
            .unwrap();
        let mut instance = factory.prepare_native(dependencies.clone(), Box::new(Increment { value: 1 }), Box::new(())).unwrap();
        let error = ready(instance.run(context.clone())).err().unwrap();
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("test.renamed_resource") && diagnostic.contains("u64"), "{diagnostic}");
        let factory = OperationFactory::function(INCREMENT_OPERATION, &[], call, |_| super::super::FailureKind::Permanent)
            .unwrap()
            .with_id("test.renamed_caller")
            .unwrap();
        let mut instance = factory.prepare_native(dependencies, Box::new(Increment { value: 1 }), Box::new(())).unwrap();
        let error = ready(instance.run(context)).err().unwrap();
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("test.renamed_caller") && diagnostic.contains("test.increment"), "{diagnostic}");
    }
}
