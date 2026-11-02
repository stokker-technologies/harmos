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
use harmos_sdk::operations::{CallFuture, Caller, InvocationError, Target};

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

struct Invocation;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Caller for HostContext {
    fn call(&self, target: Target, request: Box<dyn Any + Send>) -> CallFuture<'_> {
        Box::pin(async move {
            let result = async {
                let context = self.borrow();
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
                    let bytes = context.call_encoded(target.id, payload).await?;
                    return (target.decode)(&bytes).map_err(|source| crate::Error::OperationFailed {
                        operation: target.id.to_owned(),
                        source: Arc::new(source),
                    });
                };
                if !factory.accepts_target(&target) {
                    return Err(crate::Error::InvalidInput {
                        field: "declared operation",
                        reason: "callee payload types differ",
                    });
                }
                let grant = (target.grant)(request.as_ref());
                let key = (target.key)(request.as_ref());
                let memory = (target.memory)(request.as_ref());
                let resources = context.operations.resources_for(&entry);
                let binding = super::authored::factory_projected::<Invocation, ()>(
                    &context.operations,
                    move |request, ()| {
                        let payload = request
                            .payload
                            .into_inner()
                            .map_err(|_| harmos_sdk::ServingError::Catalog)?
                            .ok_or(harmos_sdk::ServingError::Catalog)?;
                        factory.prepare_native(resources.clone(), payload, Box::new(()))
                    },
                    |response| Some(harmos_sdk::Response::new(Answer(response))),
                    |response| response.0.clone(),
                );
                let mut input = super::binding::Request::from(Request {
                    payload: Mutex::new(Some(request)),
                    memory,
                });
                input.binding = Some(Arc::new(binding));
                input.grant = grant;
                input.key = key;
                let running = context.operations.admit_at::<Invocation, ()>(
                    target.id,
                    (input, ()),
                    Some(&context.scope),
                    context.admission.held(),
                    &context.source,
                    None,
                )?;
                let response = running.await?;
                Ok(response.0.clone())
            }
            .await;
            result.map_err(|error: crate::Error| InvocationError::failed(target.id, error))
        })
    }
}

impl OperationDeclaration for Invocation {
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
        let direct = runtime.operation(Leaf { value: 5 }).run().await.unwrap();
        let child = runtime.operation(Parent { value: 5 }).run().await.unwrap();
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

        let response = runtime.operation(Leaf { value: 5 }).run().await.unwrap();
        assert_eq!(*response, 23);
        let resources = runtime.operations.resources();
        assert!(resources.resource::<u32>().is_err());
        let owner = resources.resource::<u64>().unwrap();
        assert_eq!(*owner, 7);
        runtime.shutdown().await.unwrap();
    }
}
