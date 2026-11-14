/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Product Caller Restrictions Apply To Direct, Encoded And Plugin Child Calls.

/* === Imports === */

use super::{
    invoke_support::HostFixture,
    support::{self, Remote},
};
use futures_util::stream;
use harmos_operations::execution::operations::OperationFactory;
use harmos_protocol::host::invocations_client::InvocationsClient;
use harmos_protocol::host::operations_client::OperationsClient;
use harmos_protocol::host::time_client::TimeClient;
use harmos_protocol::host::{InvocationsCreateRequest, OperationsListRequest, OperationsListResponse, TimeGetRequest, TimeGetResponse};
use harmos_protocol::model::{Callers, Classification, Declarations, Requirement, StreamDeclaration};
use harmos_protocol::plugin::{
    ChildRequest, InstanceSamplesResponse, Sample, child_response, instance_samples_request as request,
    instance_samples_response as response,
};
use harmos_runtime::{clock::Clock, plugin::grpc::Placement};
use prost::Message;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};
use tokio::sync::mpsc;

/* === Tests === */

#[tokio::test]
async fn composition_narrows_host_callers_without_widening_or_bypassing_admission() {
    // Reject Widening And Keep The Last Accepted Composition.
    let validation = crate::support::booted(Default::default(), |setup| {
        assert!(setup.composition("[callers]\n\"harmos.host.v1.Invocations/Create\" = \"ALL\"\n").is_err());
        assert!(setup.composition("[callers]\n\"harmos.host.v1.Time/Get\" = \"PLUGINS\"\n").is_ok());
        assert!(setup.composition("[callers]\n\"harmos.host.v1.Time/Get\" = \"ALL\"\n").is_err());
        assert!(setup.composition("[callers]\n\"harmos.host.v1.Unknown/Get\" = \"CLIENTS\"\n").is_err());
    })
    .await;
    validation.shutdown().await.unwrap();

    // Narrow One Shared Operation Before Starting The Runtime.
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = HostFixture::with_composition(
        clock,
        "[callers]\n\"harmos.host.v1.Time/Get\" = \"PLUGINS\"\n",
        std::num::NonZeroUsize::new(1024 * 1024).unwrap(),
    )
    .await;
    host.runtime.operations.register_catalog(harmos_operations::host_catalog(), Default::default()).await.unwrap();
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let factories = [
        OperationFactory::protobuf::<harmos_operations::clock::Get>().unwrap(),
        OperationFactory::protobuf::<harmos_operations::lifecycle::Create>().unwrap(),
        harmos_operations::host_catalog()
            .operations
            .into_iter()
            .find(|factory| factory.definition.id.as_str() == "harmos.host.v1.Operations/List")
            .unwrap(),
    ];
    let contracts: Vec<_> = factories.iter().map(|factory| factory.contract().unwrap()).collect();

    // A Client Cannot Discover Or Invoke The Narrowed Operation Through Either Carrier.
    let listener = host
        .runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            operations: Default::default(),
            reflection: false,
            address: "127.0.0.1:0".parse().unwrap(),
            tls: None,
            connections: std::num::NonZeroUsize::new(4).unwrap(),
            calls_per_connection: std::num::NonZeroUsize::new(8).unwrap(),
            request_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
        })
        .await
        .unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut time = TimeClient::new(channel.clone());
    let refused = time.get(TimeGetRequest {}).await.unwrap_err();
    assert!(refused.to_string().contains("harmos.host.v1.Time/Get"));
    let mut discovery = OperationsClient::new(channel.clone());
    let listed = discovery.list(OperationsListRequest {}).await.unwrap().into_inner();
    assert!(
        !listed
            .operations
            .iter()
            .any(|operation| operation.contract.as_ref().is_some_and(|contract| contract.id == "harmos.host.v1.Time/Get"))
    );
    let mut invocations = InvocationsClient::new(channel);
    let refused = invocations
        .create(InvocationsCreateRequest {
            operation: "harmos.host.v1.Time/Get".into(),
            fingerprint: contracts[0].fingerprint().0.to_vec(),
            payload: vec![],
        })
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("harmos.host.v1.Time/Get"));

    // The Plugin Declares The Same Contracts And Calls Them During A Controlled Tick.
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
            assert!(matches!(input.message().await.unwrap().unwrap().frame, Some(request::Frame::Open(_))));
            let Some(request::Frame::Tick(tick)) = input.message().await.unwrap().unwrap().frame else { panic!("Expected Tick") };
            for (index, contract) in contracts.iter().enumerate() {
                output
                    .send(Ok(InstanceSamplesResponse {
                        samples: vec![],
                        frame: Some(response::Frame::Child(ChildRequest {
                            call: index as u64 + 1,
                            operation: contract.operation().id.to_string(),
                            fingerprint: contract.fingerprint().0.to_vec(),
                            payload: vec![],
                            ..Default::default()
                        })),
                    }))
                    .await
                    .unwrap();
                let Some(request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                    panic!("Expected Child Reply")
                };
                results.send(reply).unwrap();
            }
            output
                .send(Ok(InstanceSamplesResponse {
                    samples: vec![Sample {
                        at: tick.at,
                        sequence: 1,
                        row: TimeGetResponse::default().encode_to_vec(),
                    }],
                    frame: None,
                }))
                .await
                .unwrap();
            complete.store(true, Ordering::SeqCst);
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    let attached = host.runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    controller.advance_until(Duration::from_secs(2), || complete.load(Ordering::SeqCst)).await.unwrap();
    let first = observed.recv().await.unwrap();
    assert!(matches!(first.result, Some(child_response::Result::Payload(_))));
    let second = observed.recv().await.unwrap();
    let Some(child_response::Result::Failure(failure)) = second.result else { panic!("Expected Caller Refusal") };
    assert_eq!(failure.classification(), Classification::Refused);
    assert!(failure.message.contains("harmos.host.v1.Invocations/Create"));
    let third = observed.recv().await.unwrap();
    let Some(child_response::Result::Payload(payload)) = third.result else { panic!("Expected Discovery") };
    let listed = OperationsListResponse::decode(payload.as_slice()).unwrap();
    let narrowed = listed
        .operations
        .iter()
        .filter_map(|operation| operation.contract.as_ref())
        .find(|contract| contract.id == "harmos.host.v1.Time/Get")
        .unwrap();
    assert_eq!(narrowed.callers(), Callers::Plugins);
    assert!(
        listed
            .operations
            .iter()
            .any(|operation| operation.contract.as_ref().is_some_and(|contract| contract.id == "harmos.host.v1.Time/Get"))
    );
    assert!(
        !listed
            .operations
            .iter()
            .any(|operation| operation.contract.as_ref().is_some_and(|contract| contract.id == "harmos.host.v1.Invocations/Create"))
    );
    attached.stop("Story Complete").await.unwrap();
    drop(listener);
    host.runtime.shutdown().await.unwrap();
}
