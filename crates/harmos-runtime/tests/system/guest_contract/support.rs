/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Guest Story Fixtures

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::contract::{Echoed, ProviderEchoOperation, RetryChoice, Stage};
use harmos_runtime::{
    Runtime,
    operations::{ExecutionContext, ProtobufOperation, Resource, ServingError, Visibility},
};
use prost::Message as _;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub struct ProviderOwner {
    pub observed: Mutex<Vec<String>>,
    pub prepared: AtomicUsize,
    pub started: tokio::sync::Notify,
    pub cancelled: Arc<tokio::sync::Notify>,
}
struct Provider;
pub struct Publisher;
struct Cancelled(Arc<tokio::sync::Notify>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Drop for Cancelled {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}
fn descriptors() -> &'static prost_reflect::DescriptorPool {
    static POOL: std::sync::OnceLock<prost_reflect::DescriptorPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let mut types = <ProviderEchoOperation as harmos::Message>::types();
        harmos::encoding::merge_descriptor_types(&mut types, <Echoed as harmos::Message>::types());
        prost_reflect::DescriptorPool::from_file_descriptor_set(types).unwrap()
    })
}
impl prost_reflect::ReflectMessage for ProviderEchoOperation {
    fn descriptor(&self) -> prost_reflect::MessageDescriptor {
        descriptors().get_message_by_name(&<Self as harmos::Message>::name()).unwrap()
    }
}
impl prost_reflect::ReflectMessage for Echoed {
    fn descriptor(&self) -> prost_reflect::MessageDescriptor {
        descriptors().get_message_by_name(&<Self as harmos::Message>::name()).unwrap()
    }
}
impl ProtobufOperation for Provider {
    type Request = ProviderEchoOperation;
    type Response = Echoed;
    type Endpoints = ();
    const ID: &'static str = "provider.echo";
    const POLICY: harmos_runtime::OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: std::time::Duration::from_millis(100),
        attempt_timeout: std::time::Duration::from_millis(100),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: harmos_runtime::RetryPolicy::Never,
    };
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<ProviderOwner>(), Resource::of::<harmos_runtime::operations::HostContext>()];
    fn grant(request: &Self::Request) -> Result<Option<harmos_operations::plugin::Grant>, harmos_runtime::MessageError> {
        Ok(request.say.strip_prefix("scope:").map(|scope| harmos_operations::plugin::Grant::of(Self::ID, scope.to_owned())))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let owner = context.resource::<ProviderOwner>().map_err(|error| ServingError::Native(Arc::new(error)))?;
        let host = context.resource::<harmos_runtime::operations::HostContext>().map_err(|error| ServingError::Native(Arc::new(error)))?;
        owner.prepared.fetch_add(1, Ordering::SeqCst);
        if request.say == "cancel" {
            let _cancelled = Cancelled(Arc::clone(&owner.cancelled));
            owner.started.notify_one();
            host.borrow().scope().clock().sleep(std::time::Duration::from_secs(60)).await;
        }
        if request.say == "timeout" {
            host.borrow().scope().clock().sleep(std::time::Duration::from_secs(60)).await;
        }
        if request.say == "refuse" {
            return Err(ServingError::Native(Arc::new(harmos_operations::Refusal::new(73, "provider refusal"))));
        }
        owner.observed.lock().unwrap().push(request.say.clone());
        Ok(Echoed { said: request.say })
    }
}
impl ProtobufOperation for Publisher {
    type Request = harmos_protocol::context::SamplesAppendRequest;
    type Response = harmos_protocol::context::SamplesAppendResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Samples/Append";
    const POLICY: harmos_runtime::OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<harmos_runtime::operations::HostContext>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().map_err(|error| ServingError::Native(Arc::new(error)))?;
        let context = host.borrow();
        let scope = context.caller_scope().ok_or_else(|| {
            ServingError::Native(Arc::new(harmos_runtime::Error::<std::convert::Infallible>::InvalidInput {
                field: "guest publication",
                reason: "requires the original generation scope",
            }))
        })?;
        if request.sequence.is_some() || request.published_at.is_some() || request.context.is_some() || !request.following.is_empty() {
            return Err(ServingError::Native(Arc::new(harmos_runtime::Error::<std::convert::Infallible>::InvalidInput {
                field: "guest publication",
                reason: "sequence, batch and publication metadata are host-derived",
            })));
        }
        let lease = scope.publication(&request.stream).map_err(|error| ServingError::Native(Arc::new(error)))?;
        lease
            .stage_derived_row(
                harmos_runtime::streams::PublicationRow {
                    payload: request.payload,
                    published_unix_micros: None,
                    context: None,
                },
                0,
            )
            .map_err(|error| ServingError::Native(Arc::new(error)))?;
        Ok(harmos_protocol::context::SamplesAppendResponse {})
    }
}
impl harmos_runtime::Message for Stage {
    const CODEC: &'static str = "harmos.protobuf.v1";
    fn encode(&self) -> Vec<u8> {
        self.encode_to_vec()
    }
    fn encoded_size(&self) -> usize {
        self.encoded_len()
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <Self as prost::Message>::decode(bytes).map_err(|error| harmos_runtime::MessageError::new(error.to_string()))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        Ok(bytes.len() + std::mem::size_of::<Self>())
    }
}
impl harmos_runtime::operations::ContextValue for Stage {
    const KEY: &'static str = "harmos.sandbox.v1.Stage";
}
impl harmos_runtime::Message for RetryChoice {
    const CODEC: &'static str = "harmos.protobuf.v1";
    fn encode(&self) -> Vec<u8> {
        self.encode_to_vec()
    }
    fn encoded_size(&self) -> usize {
        self.encoded_len()
    }
    fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
        <Self as prost::Message>::decode(bytes).map_err(|error| harmos_runtime::MessageError::new(error.to_string()))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
        Ok(bytes.len() + std::mem::size_of::<Self>())
    }
}
impl harmos_runtime::operations::ContextValue for RetryChoice {
    const KEY: &'static str = "harmos.sandbox.v1.RetryChoice";
}

/* === Public Functions === */

pub async fn runtime(
    owner: Arc<ProviderOwner>, lane: harmos_runtime::operations::Lane, clock: harmos_runtime::clock::Clock,
) -> Runtime<crate::support::Bank> {
    let runtime = Runtime::<crate::support::Bank>::launch_with(crate::support::Ledger::default(), (), |setup| {
        setup.resource::<ProviderOwner>(owner);
        setup.settings().clock(clock);
        setup.settings().register(crate::support::catalog());
        setup.operation(harmos_runtime::operations::OperationFactory::protobuf::<Publisher>().unwrap(), lane);
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    runtime
        .operations
        .register_operation(harmos_runtime::operations::OperationFactory::protobuf::<Provider>().unwrap(), lane)
        .await
        .unwrap();
    runtime
}
pub fn host(runtime: &Runtime<crate::support::Bank>) -> harmos_runtime::guest::Host<crate::support::Bank> {
    harmos_runtime::guest::Host::with_settings(
        runtime,
        harmos_runtime::config::PluginGuestSettings {
            stream_window: NonZeroUsize::new(4),
            ..Default::default()
        },
    )
    .unwrap()
}
pub fn settings(configuration: &str) -> harmos_runtime::config::PluginSettings {
    harmos_runtime::config::PluginSettings {
        configuration: serde_json::to_vec(configuration).unwrap(),
        host_operations: ["harmos.context.v1.Samples/Append", "provider.echo"]
            .into_iter()
            .map(|id| harmos_runtime::Identifier::new(id).unwrap())
            .collect(),
        ..Default::default()
    }
}
pub fn sandbox() -> Vec<u8> {
    crate::support::fixtures::component("harmos-sandbox-guest")
}
pub async fn call<Q: harmos::OperationRequest>(
    runtime: &harmos_runtime::operations::OperationManager, request: Q,
) -> harmos_runtime::Result<Q::Response> {
    let bytes = runtime.call_encoded(Q::ID, request.encode_to_vec()).await?;
    <Q::Response as prost::Message>::decode(bytes.as_slice()).map_err(|source| harmos_runtime::Error::OperationPayload {
        operation: Q::ID.into(),
        source: harmos_runtime::MessageError::new(source.to_string()),
    })
}
pub fn component_error(error: &harmos_runtime::Error) -> &harmos_runtime::operations::ComponentError {
    let harmos_runtime::Error::OperationFailed { source, .. } = error else { panic!("expected the component failure, got {error:?}") };
    source.downcast_ref().expect("component error retains its type")
}

/* === Private Functions === */

/* === Tests === */
