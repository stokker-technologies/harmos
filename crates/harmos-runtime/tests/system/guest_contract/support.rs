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

use crate::backend::{Backend, RecoveryBackend};
use crate::support::Bank;
use harmos::OperationRequest;
use harmos_operations::plugin::Grant;
use harmos_runtime::plugin::{
    grpc::PlacementSettings,
    manager::{self, Manager, PluginManagerSettings},
};
use harmos_runtime::storage::{StorageBackends, StorageManager};
use harmos_runtime::{
    Error, Result as RuntimeResult,
    clock::Clock,
    config::{PluginGuestSettings, PluginSettings},
    guest::Host,
};
use tempfile::TempDir;

use super::contract::{Echoed, ProviderEchoOperation, PublishProgress, RetryChoice, Stage};
use harmos_protocol::model::Callers;
use harmos_protocol::schema::with_method_callers;
use harmos_runtime::{
    MessageError, Runtime,
    operations::{ComponentError, ExecutionContext, Lane, OperationManager, ProtobufOperation, Resource, ServingError, Visibility},
};
use prost::Message as _;
use prost_reflect::{DescriptorPool, MessageDescriptor};
use prost_types::{MethodDescriptorProto, ServiceDescriptorProto};
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
pub(super) struct Provider;
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
fn descriptors() -> &'static DescriptorPool {
    static POOL: std::sync::OnceLock<prost_reflect::DescriptorPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let mut types = <ProviderEchoOperation as harmos::Message>::types();
        harmos::encoding::merge_descriptor_types(&mut types, <Echoed as harmos::Message>::types());
        harmos::encoding::merge_descriptor_types(&mut types, <PublishProgress as harmos::Message>::types());
        harmos::encoding::merge_descriptor_types(&mut types, <() as harmos::Message>::types());
        let file = types.file.iter_mut().find(|file| file.package.as_deref() == Some("example.host.v1")).unwrap();
        file.dependency.push("google/protobuf/empty.proto".into());
        file.service.push(ServiceDescriptorProto {
            name: Some("Progress".into()),
            method: vec![MethodDescriptorProto {
                name: Some("Publish".into()),
                input_type: Some(".example.host.v1.PublishProgress".into()),
                output_type: Some(".google.protobuf.Empty".into()),
                options: Some(Default::default()),
                ..Default::default()
            }],
            ..Default::default()
        });
        with_method_callers(types, &[("example.host.v1.Progress/Publish".into(), Callers::Plugins)].into_iter().collect()).unwrap()
    })
}
impl prost_reflect::ReflectMessage for ProviderEchoOperation {
    fn descriptor(&self) -> MessageDescriptor {
        descriptors().get_message_by_name(&<Self as harmos::Message>::name()).unwrap()
    }
}
impl prost_reflect::ReflectMessage for Echoed {
    fn descriptor(&self) -> MessageDescriptor {
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
    fn grant(request: &Self::Request) -> Result<Option<Grant>, MessageError> {
        Ok(request.say.strip_prefix("scope:").map(|scope| Grant::of(Self::ID, scope.to_owned())))
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
impl prost_reflect::ReflectMessage for PublishProgress {
    fn descriptor(&self) -> MessageDescriptor {
        descriptors().get_message_by_name(&<Self as harmos::Message>::name()).unwrap()
    }
}
impl ProtobufOperation for Publisher {
    type Request = PublishProgress;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "example.host.v1.Progress/Publish";
    const POLICY: harmos_runtime::OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<harmos_runtime::operations::HostContext>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let host = context.resource::<harmos_runtime::operations::HostContext>().map_err(|error| ServingError::Native(Arc::new(error)))?;
        let context = host.borrow();
        let scope = context.caller_scope().ok_or_else(|| {
            ServingError::Native(Arc::new(Error::<std::convert::Infallible>::InvalidInput {
                field: "guest publication",
                reason: "requires the original generation scope",
            }))
        })?;
        let lease = scope.publication(&request.stream).map_err(|error| ServingError::Native(Arc::new(error)))?;
        let sample = Echoed::decode(request.payload.as_slice())
            .map_err(|error| ServingError::Codec(MessageError::new(format!("Publish Progress: {error}"))))?;
        let payload = harmos_runtime::Message::encode(&sample.said);
        lease
            .stage_derived_row(
                harmos_runtime::streams::PublicationRow {
                    payload,
                    published_unix_micros: None,
                    context: None,
                },
                0,
            )
            .map_err(|error| ServingError::Native(Arc::new(error)))?;
        Ok(())
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
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        <Self as prost::Message>::decode(bytes).map_err(|error| MessageError::new(error.to_string()))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
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
    fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
        <Self as prost::Message>::decode(bytes).map_err(|error| MessageError::new(error.to_string()))
    }
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
        Ok(bytes.len() + std::mem::size_of::<Self>())
    }
}
impl harmos_runtime::operations::ContextValue for RetryChoice {
    const KEY: &'static str = "harmos.sandbox.v1.RetryChoice";
}

/* === Public Functions === */

pub async fn runtime(owner: Arc<ProviderOwner>, lane: Lane, clock: Clock) -> Arc<Runtime<Bank>> {
    let (storage, entries, root) = entries();
    let runtime = Runtime::<Bank>::launch_with(crate::support::Ledger::default(), (), |setup| {
        setup.resource::<ProviderOwner>(owner);
        setup.resource(root);
        setup.settings().storage(&storage).plugins(entries.clone());
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
    let runtime = Arc::new(runtime);
    entries.start(&runtime).await.unwrap();
    runtime
        .operations
        .register_operation(harmos_runtime::operations::OperationFactory::protobuf::<Provider>().unwrap(), lane)
        .await
        .unwrap();
    runtime
}
pub fn entries() -> (StorageManager, Manager, TempDir) {
    use std::time::Duration;
    let root = tempfile::tempdir().unwrap();
    let catalog = manager::catalog();
    let storage = StorageManager::with_catalog(
        catalog.clone(),
        StorageBackends {
            primary: Arc::new(Backend::with_catalog(catalog, crate::backend::history())),
            state: Arc::new(RecoveryBackend::new(crate::backend::history())),
        },
        crate::backend::storage_settings(),
    )
    .unwrap();
    let settings = PluginManagerSettings {
        namespace: "guest-entries".into(),
        directory: root.path().join("plugins"),
        target: if cfg!(target_os = "windows") { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }.into(),
        placement: PlacementSettings {
            connect_timeout: Duration::from_secs(10),
            reconnect_delay: Duration::from_millis(100),
            stop_grace: Duration::from_secs(10),
            stream_window: NonZeroUsize::new(4).unwrap(),
        },
        bundle: Default::default(),
        entries: NonZeroUsize::new(16).unwrap(),
        create_receipts: NonZeroUsize::new(64).unwrap(),
        registry_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        command_capacity: NonZeroUsize::new(16).unwrap(),
        retained_cuts: NonZeroUsize::new(64).unwrap(),
        watch_capacity: NonZeroUsize::new(16).unwrap(),
        response_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
    };
    let manager = Manager::new(storage.clone(), settings).unwrap();
    (storage, manager, root)
}

pub fn host(runtime: &Runtime<Bank>) -> Host<Bank> {
    Host::with_settings(
        runtime,
        PluginGuestSettings {
            stream_window: NonZeroUsize::new(4),
            ..Default::default()
        },
    )
    .unwrap()
}
pub fn settings(configuration: &str) -> PluginSettings {
    PluginSettings {
        configuration: serde_json::to_vec(configuration).unwrap(),
        host_operations: ["example.host.v1.Progress/Publish", "provider.echo", "harmos.host.v1.Time/Get"]
            .into_iter()
            .map(|id| harmos_runtime::Identifier::new(id).unwrap())
            .collect(),
        ..Default::default()
    }
}
pub fn sandbox() -> Vec<u8> {
    crate::support::fixtures::component("harmos-sandbox-guest")
}
pub async fn call<Q: OperationRequest>(runtime: &OperationManager, request: Q) -> RuntimeResult<Q::Response> {
    let bytes = runtime.call_encoded(Q::ID, request.encode_to_vec()).await?;
    <Q::Response as prost::Message>::decode(bytes.as_slice()).map_err(|source| Error::OperationPayload {
        operation: Q::ID.into(),
        source: MessageError::new(source.to_string()),
    })
}
pub fn component_error(error: &Error) -> &ComponentError {
    let Error::OperationFailed { source, .. } = error else { panic!("expected the component failure, got {error:?}") };
    source.downcast_ref().expect("component error retains its type")
}

/* === Private Functions === */

/* === Tests === */
