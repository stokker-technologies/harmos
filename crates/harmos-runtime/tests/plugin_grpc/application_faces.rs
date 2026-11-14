/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// An Application Composes Its Own Faces Beside the Library Services.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_operations::OperationPolicy;
use harmos_operations::execution::operations::{ExecutionContext, OperationFactory, ProtobufOperation, ServingError, Visibility};
use harmos_protocol::plugin::host::{TimeGetRequest, TimeGetResponse};
use harmos_runtime::plugin::grpc::Placement;
use prost::Message as _;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Prompts;
struct Steps;
struct Control;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Prompts {
    type Request = TimeGetRequest;
    type Response = TimeGetResponse;
    type Endpoints = ();
    const ID: &'static str = "example.plugin.host.v1.Prompts/Create";
    const POLICY: OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, _: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        Ok(Self::Response {
            now: Some(prost_types::Timestamp { seconds: 17, nanos: 0 }),
        })
    }
}

impl ProtobufOperation for Steps {
    type Request = TimeGetRequest;
    type Response = TimeGetResponse;
    type Endpoints = ();
    const ID: &'static str = "example.client.v1.Steps/Execute";
    const POLICY: OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, _: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        Ok(Self::Response {
            now: Some(prost_types::Timestamp { seconds: 17, nanos: 0 }),
        })
    }
}

impl ProtobufOperation for Control {
    type Request = TimeGetRequest;
    type Response = TimeGetResponse;
    type Endpoints = ();
    const ID: &'static str = "example.plugin.v1.Control/Execute";
    const POLICY: OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    async fn run(_: ExecutionContext, _: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        Ok(Self::Response {
            now: Some(prost_types::Timestamp { seconds: 17, nanos: 0 }),
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn application_faces_admit_context_calls_and_refuse_other_audiences() {
    // Compose Application Declarations Without Client Bindings for Context or Instance.
    let host = HostFixture::new().await;
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let factories = [
        OperationFactory::protobuf::<Prompts>().unwrap(),
        OperationFactory::protobuf::<Steps>().unwrap(),
        OperationFactory::protobuf::<Control>().unwrap(),
    ];
    let contracts: Vec<_> = factories.iter().map(|factory| factory.contract().unwrap()).collect();
    let registration = harmos_runtime::operations::Registration::new(Default::default())
        .entry(factories[0].clone())
        .entry(factories[1].clone())
        .entry(factories[2].clone());
    host.runtime.register_batch(registration).await.unwrap();
    assert_eq!(host.runtime.operations.call_protobuf::<Prompts>(Default::default()).await.unwrap().now.unwrap().seconds, 17);
    assert_eq!(host.runtime.operations.call_protobuf::<Control>(Default::default()).await.unwrap().now.unwrap().seconds, 17);

    // A Plugin Declares Exact Contracts but Receives Only Context Authority.
    let caller = host.sdk("caller").await;
    let _caller = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();
    assert_eq!(
        host.runtime.operations.call_encoded("fixture.forward", payload(b"application-context")).await.unwrap(),
        payload(&17_i64.to_le_bytes())
    );
    for (mode, id) in [(b"application-client".as_slice(), Steps::ID), (b"application-instance", Control::ID)] {
        let error = host.runtime.operations.call_encoded("fixture.forward", payload(mode)).await.unwrap_err();
        assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
        assert!(call_error(&error).to_string().contains(id));
    }

    // A Client Uses the Generic Carrier and Cannot Acquire Either Other Face.
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
            request_timeout: std::time::Duration::from_secs(2),
            shutdown_timeout: std::time::Duration::from_secs(2),
        })
        .await
        .unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut client = harmos_protocol::client::invocations_client::InvocationsClient::new(channel);
    for contract in &contracts {
        let result = client
            .invoke(harmos_protocol::client::InvocationsInvokeRequest {
                operation: contract.operation().id.to_string(),
                fingerprint: contract.fingerprint().0.to_vec(),
                payload: Vec::new(),
            })
            .await;
        if contract.operation().id.as_str() == Steps::ID {
            let response = TimeGetResponse::decode(result.unwrap().into_inner().payload.as_slice()).unwrap();
            assert_eq!(response.now.unwrap().seconds, 17);
        } else {
            assert!(result.unwrap_err().to_string().contains(contract.operation().id.as_str()));
        }
    }
    drop(listener);
    host.runtime.shutdown().await.unwrap();
}
