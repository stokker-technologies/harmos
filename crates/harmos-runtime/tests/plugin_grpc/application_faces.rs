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
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

// Third Party Imports
use futures_util::stream;
use prost::Message;
use tokio::sync::mpsc;

// Local Imports
use super::{
    invoke_support::HostFixture,
    support::{self, Remote},
};
use harmos_operations::execution::operations::{
    ExecutionContext, OperationFactory, ProtobufOperation, Resource, ServingError, Visibility, access::DiscoveryAccess,
};
use harmos_protocol::host::{OperationsGetRequest, OperationsGetResponse, OperationsListRequest, OperationsListResponse, TimeGetResponse};
use harmos_protocol::model::{Callers, Classification, Declarations, Requirement, StreamDeclaration};
use harmos_protocol::plugin::{ChildRequest, InstanceSamplesResponse, child_response, instance_samples_request, instance_samples_response};
use harmos_runtime::{clock::Clock, plugin::grpc::Placement};

/* === Structs === */

struct DiscoveryList;
struct DiscoveryGet;

/* === Implementations === */

impl ProtobufOperation for DiscoveryList {
    type Request = OperationsListRequest;
    type Response = OperationsListResponse;
    type Endpoints = ();
    const ID: &'static str = "fixture.discovery.list";
    const POLICY: harmos_runtime::OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<DiscoveryAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, endpoints: &()) -> Result<Self::Response, ServingError> {
        harmos_operations::host::List::run(context, request, endpoints).await
    }
}

impl ProtobufOperation for DiscoveryGet {
    type Request = OperationsGetRequest;
    type Response = OperationsGetResponse;
    type Endpoints = ();
    const ID: &'static str = "fixture.discovery.get";
    const POLICY: harmos_runtime::OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<DiscoveryAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, endpoints: &()) -> Result<Self::Response, ServingError> {
        harmos_operations::host::Get::run(context, request, endpoints).await
    }
}

/* === Tests === */

#[tokio::test]
async fn harmos_host_operations_are_unreachable_on_the_wire() {
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::configured(Some(clock), std::num::NonZeroUsize::new(1024 * 1024).unwrap(), None).await;
    host.runtime.operations.register_catalog(harmos_operations::host_catalog(), Default::default()).await.unwrap();
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    host.runtime
        .register_batch(
            harmos_runtime::operations::Registration::new(Default::default())
                .entry(OperationFactory::protobuf::<DiscoveryList>().unwrap())
                .entry(OperationFactory::protobuf::<DiscoveryGet>().unwrap()),
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
        types: Some(harmos_protocol::clock::declarations_types()),
        streams: vec![StreamDeclaration {
            id: "caller_probe".into(),
            row: "harmos.host.v1.TimeGetResponse".into(),
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
            output.send(Ok(support::answer(1, TimeGetResponse { now: window.start }.encode_to_vec()))).await.unwrap();
            complete.store(true, Ordering::SeqCst);
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let _watching = host.runtime.streams.current_publication("caller_probe").unwrap().watch();
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
    host.runtime.shutdown().await.unwrap();
}
