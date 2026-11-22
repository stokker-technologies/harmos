/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Scoped Discovery Wrappers Apply the Same Predicate as Dispatch.

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use prost::Message;
use prost_reflect::{MessageDescriptor, ReflectMessage};
use tokio::sync::mpsc;

// Local Imports
use super::{
    invoke_support::HostFixture,
    support::{self, Remote},
};
use harmos_operations::execution::operations::{
    ExecutionContext, NativeOperation, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility, access::CallingPlugin,
};
use harmos_operations::host::declarations::host;
use harmos_protocol::host::{OperationsGetRequest, OperationsGetResponse, OperationsListRequest, OperationsListResponse};
use harmos_protocol::model::{Callers, Classification, Declarations, InstanceState, Requirement, StreamDeclaration};
use harmos_protocol::plugin::{ChildRequest, InstanceSamplesResponse, child_response, instance_samples_request, instance_samples_response};
use harmos_runtime::{OperationPolicy, clock::Clock, operations::HostContext, plugin::grpc::Placement};

/* === Structs === */

struct DiscoveryList;
struct DiscoveryGet;
struct NativeCaller;

/* === Implementations === */

impl ProtobufOperation for DiscoveryList {
    type Request = OperationsListRequest;
    type Response = OperationsListResponse;
    type Endpoints = ();
    const ID: &'static str = "fixture.discovery.list";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let caller = context.resource::<CallingPlugin>().unwrap();
        let parent = context.resource::<HostContext>().unwrap();
        let call = parent.for_call();
        let native = call.call::<NativeCaller>(()).await.map_err(|error| ServingError::Native(Arc::new(error)))?;
        assert_eq!(native, caller.name);
        call.call::<host::List>(request).await.map_err(|error| ServingError::Native(Arc::new(error)))
    }
}

impl ProtobufOperation for DiscoveryGet {
    type Request = OperationsGetRequest;
    type Response = OperationsGetResponse;
    type Endpoints = ();
    const ID: &'static str = "fixture.discovery.get";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<HostContext>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let parent = context.resource::<HostContext>().unwrap();
        parent.for_call().call::<host::Get>(request).await.map_err(|error| ServingError::Native(Arc::new(error)))
    }
}

impl NativeOperation for NativeCaller {
    type Request = ();
    type Response = Option<String>;
    type Endpoints = ();
    const ID: &'static str = "fixture.native_caller";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;

    async fn run(context: ExecutionContext, _: (), _: &()) -> Result<Self::Response, ServingError> {
        let caller = context.resource::<CallingPlugin>().map_err(|_| ServingError::Catalog)?;
        Ok(caller.name.clone())
    }
}

/* === Tests === */

#[tokio::test]
async fn harmos_host_operations_are_unreachable_on_the_wire() {
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::configured(Some(clock), NonZeroUsize::new(1024 * 1024).unwrap(), None).await;
    host.runtime.operations.register_catalog(harmos_operations::host_catalog(), Default::default()).await.unwrap();
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    host.runtime
        .register_batch(
            harmos_runtime::operations::Registration::new(Default::default())
                .entry(OperationFactory::protobuf::<DiscoveryList>().unwrap())
                .entry(OperationFactory::protobuf::<DiscoveryGet>().unwrap())
                .entry(OperationFactory::native::<NativeCaller>().unwrap()),
        )
        .await
        .unwrap();
    let factories = [
        OperationFactory::protobuf::<harmos_operations::facilities::TimeGet>().unwrap(),
        OperationFactory::protobuf::<harmos_operations::lifecycle::Create>().unwrap(),
        OperationFactory::protobuf::<DiscoveryList>().unwrap(),
        OperationFactory::protobuf::<DiscoveryGet>().unwrap(),
    ];
    let contracts: Vec<_> = factories.iter().map(|factory| factory.contract().unwrap()).collect();

    // The Plugin Declares The Same Contracts And Calls Them During A Controlled Window.
    let remote = Remote::start().await;
    let mut declarations = Declarations {
        requires: contracts
            .iter()
            .map(|contract| Requirement {
                id: contract.operation().id.to_string(),
                fingerprint: contract.fingerprint().0.to_vec(),
            })
            .collect(),
        types: Some(support::clock_types()),
        streams: vec![StreamDeclaration {
            id: "fixture.caller_probe".into(),
            row: "fixture.ClockRow".into(),
            interval: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        }],
        ..Default::default()
    };
    declarations.digest = harmos_protocol::schema::declarations_digest(&declarations);
    remote.declarations(declarations);
    let (results, mut observed) = mpsc::unbounded_channel();
    let complete = Arc::new(AtomicBool::new(false));
    let completed = complete.clone();
    remote.samples(move |mut input| {
        let (output, receiver) = mpsc::channel(4);
        let results = results.clone();
        let complete = completed.clone();
        let contracts = contracts.clone();
        tokio::spawn(async move {
            assert!(matches!(input.message().await.unwrap().unwrap().frame, Some(instance_samples_request::Frame::Open(_))));
            support::acknowledge_demand(&mut input, &output).await;
            let Some(instance_samples_request::Frame::Window(window)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected a Window")
            };
            for (index, contract) in contracts.iter().enumerate().chain(std::iter::once((contracts.len(), &contracts[3]))) {
                let payload = if index >= 3 {
                    OperationsGetRequest {
                        id: if index == 3 { "harmos.plugin.v1.Time/Sleep" } else { "harmos.plugin.v1.Time/Get" }.into(),
                    }
                    .encode_to_vec()
                } else {
                    vec![]
                };
                output
                    .send(Ok(InstanceSamplesResponse {
                        samples: vec![],
                        frame: Some(instance_samples_response::Frame::Child(ChildRequest {
                            call: index as u64 + 1,
                            operation: contract.operation().id.to_string(),
                            fingerprint: contract.fingerprint().0.to_vec(),
                            payload,
                            ..Default::default()
                        })),
                    }))
                    .await
                    .unwrap();
                let Some(instance_samples_request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                    panic!("Expected Child Reply")
                };
                results.send(reply).unwrap();
            }
            output.send(Ok(support::answer(1, support::ClockRow { now: window.start }.encode_to_vec()))).await.unwrap();
            complete.store(true, Ordering::SeqCst);
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let _watching = host.runtime.streams.current_publication("fixture.caller_probe").unwrap().watch();
    controller.advance_until(Duration::from_secs(2), || complete.load(Ordering::SeqCst)).await.unwrap();
    let first = observed.recv().await.unwrap();
    assert!(matches!(first.result, Some(child_response::Result::Payload(_))));
    let second = observed.recv().await.unwrap();
    let Some(child_response::Result::Failure(failure)) = second.result else { panic!("Expected Caller Refusal") };
    assert_eq!(failure.classification(), Classification::Refused);
    assert_eq!(failure.code, harmos_operations::WRONG_CALLER.to_string());
    assert_eq!(failure.action, "Call");
    assert!(failure.message.contains("harmos.host.v1.Invocations/Create"));
    let third = observed.recv().await.unwrap();
    let Some(child_response::Result::Payload(payload)) = third.result else { panic!("Expected Discovery") };
    let listed = OperationsListResponse::decode(payload.as_slice()).unwrap();
    let declared = listed
        .operations
        .iter()
        .filter_map(|operation| operation.contract.as_ref())
        .find(|contract| contract.id == "harmos.plugin.v1.Time/Get")
        .unwrap();
    assert_eq!(declared.callers(), Callers::Plugins);
    assert!(
        listed
            .operations
            .iter()
            .any(|operation| operation.contract.as_ref().is_some_and(|contract| contract.id == "harmos.plugin.v1.Time/Get"))
    );
    assert!(
        !listed
            .operations
            .iter()
            .any(|operation| operation.contract.as_ref().is_some_and(|contract| contract.id == "harmos.host.v1.Invocations/Create"))
    );
    assert!(
        !listed
            .operations
            .iter()
            .any(|operation| operation.contract.as_ref().is_some_and(|contract| contract.id == "harmos.plugin.v1.Time/Sleep"))
    );
    let fourth = observed.recv().await.unwrap();
    assert!(matches!(fourth.result, Some(child_response::Result::Failure(_))));
    let fifth = observed.recv().await.unwrap();
    let Some(child_response::Result::Payload(payload)) = fifth.result else { panic!("Expected Declared Operation") };
    let found = OperationsGetResponse::decode(payload.as_slice()).unwrap();
    assert_eq!(found.operation.unwrap().contract.unwrap().id, "harmos.plugin.v1.Time/Get");
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

struct JsonEcho;
struct JsonScalar;

#[derive(Clone, PartialEq, Message)]
struct DurationValue {
    #[prost(int64, tag = "1")]
    seconds: i64,

    #[prost(int32, tag = "2")]
    nanos: i32,
}

impl ReflectMessage for DurationValue {
    fn descriptor(&self) -> MessageDescriptor {
        InstanceState::default().descriptor().parent_pool().get_message_by_name("google.protobuf.Duration").unwrap()
    }
}

impl ProtobufOperation for JsonEcho {
    type Request = InstanceState;
    type Response = InstanceState;
    type Endpoints = ();
    const ID: &'static str = "fixture.json";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;

    async fn run(_: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        Ok(request)
    }
}

impl ProtobufOperation for JsonScalar {
    type Request = DurationValue;
    type Response = DurationValue;
    type Endpoints = ();
    const ID: &'static str = "fixture.scalar";
    const POLICY: OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;

    async fn run(_: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        Ok(request)
    }
}

#[tokio::test]
async fn untyped_children_use_the_served_contract_and_typed_invocation() {
    // Register a Typed Handler and a Deterministic Host Clock.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::configured(Some(clock), NonZeroUsize::new(1024 * 1024).unwrap(), None).await;
    host.runtime
        .register_batch(
            harmos_runtime::operations::Registration::new(Default::default())
                .entry(OperationFactory::protobuf::<JsonEcho>().unwrap())
                .entry(OperationFactory::protobuf::<JsonScalar>().unwrap()),
        )
        .await
        .unwrap();

    // The Plugin Has No Typed Requirements for These JSON Calls.
    let remote = Remote::start().await;
    let mut declarations = Declarations {
        types: Some(support::clock_types()),
        streams: vec![StreamDeclaration {
            id: "fixture.json_probe".into(),
            row: "fixture.ClockRow".into(),
            interval: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        }],
        ..Default::default()
    };
    declarations.digest = harmos_protocol::schema::declarations_digest(&declarations);
    remote.declarations(declarations);
    let (results, mut observed) = mpsc::unbounded_channel();
    let complete = Arc::new(AtomicBool::new(false));
    let completed = complete.clone();
    remote.samples(move |mut input| {
        let (output, receiver) = mpsc::channel(4);
        let results = results.clone();
        let complete = completed.clone();
        tokio::spawn(async move {
            assert!(matches!(input.message().await.unwrap().unwrap().frame, Some(instance_samples_request::Frame::Open(_))));
            support::acknowledge_demand(&mut input, &output).await;
            let Some(instance_samples_request::Frame::Window(window)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected a Window")
            };
            for (index, (operation, text)) in [
                ("fixture.json", r#"{"phase":"wifi","cancellation_requested":true}"#),
                ("fixture.json", "{}"),
                ("fixture.scalar", r#"{"value":"1.500s"}"#),
                ("fixture.json", r#"{"extra":true}"#),
                ("fixture.json", r#"{"phase":false}"#),
                ("fixture.json", "false"),
                ("fixture.absent", "{}"),
            ]
            .into_iter()
            .enumerate()
            {
                output
                    .send(Ok(InstanceSamplesResponse {
                        samples: vec![],
                        frame: Some(instance_samples_response::Frame::Child(ChildRequest {
                            call: index as u64 + 1,
                            operation: operation.into(),
                            json: Some(text.into()),
                            ..Default::default()
                        })),
                    }))
                    .await
                    .unwrap();
                let Some(instance_samples_request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                    panic!("Expected Child Reply")
                };
                results.send(reply).unwrap();
            }
            output.send(Ok(support::answer(1, support::ClockRow { now: window.start }.encode_to_vec()))).await.unwrap();
            complete.store(true, Ordering::SeqCst);
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });

    // Invoke Through the Host and Preserve Proto Names and Default Fields.
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let _watching = host.runtime.streams.current_publication("fixture.json_probe").unwrap().watch();
    controller.advance_until(Duration::from_secs(2), || complete.load(Ordering::SeqCst)).await.unwrap();
    let Some(child_response::Result::Payload(payload)) = observed.recv().await.unwrap().result else { panic!("Expected JSON Reply") };
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&payload).unwrap(),
        serde_json::json!({
            "phase":"wifi", "outcome":"", "cancellation_requested":true, "attempt":0
        })
    );
    let Some(child_response::Result::Payload(payload)) = observed.recv().await.unwrap().result else {
        panic!("Expected Default JSON Reply")
    };
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&payload).unwrap(),
        serde_json::json!({
            "phase":"", "outcome":"", "cancellation_requested":false, "attempt":0
        })
    );
    let Some(child_response::Result::Payload(payload)) = observed.recv().await.unwrap().result else {
        panic!("Expected Wrapped Scalar Reply")
    };
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&payload).unwrap(), serde_json::json!({"value":"1.500s"}));
    for (classification, message) in [
        (
            Classification::Invalid,
            "Call fixture.json: Field \"extra\" Is Not in Its Request; Expected phase, outcome, cancellation_requested, attempt",
        ),
        (Classification::Invalid, "Call fixture.json: Field \"phase\" Must Be String"),
        (Classification::Invalid, "Call fixture.json: Supply a JSON Object for the Request"),
        (Classification::Refused, "Call fixture.absent: No Plugin Serves This Operation; Install or Enable the Plugin That Provides It"),
    ] {
        let Some(child_response::Result::Failure(failure)) = observed.recv().await.unwrap().result else { panic!("Expected JSON Failure") };
        assert_eq!(failure.classification(), classification);
        assert_eq!(failure.message, message);
    }
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
