/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protected Built-In Admission Story

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_operations::execution::operations::{ExecutionContext, OperationFactory, ProtobufOperation, ServingError, Visibility};
use harmos_runtime::plugin::grpc::Placement;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Control;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Control {
    type Request = harmos_protocol::context::TimeGetRequest;
    type Response = harmos_protocol::context::TimeGetResponse;
    type Endpoints = ();
    const ID: &'static str = "fixture.control";
    const POLICY: harmos_operations::OperationPolicy = harmos_operations::lifecycle::POLICY;
    const VISIBILITY: Visibility = Visibility::Protected;
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
async fn protected_host_operation_refuses_a_plugin_with_a_matching_requirement() {
    // Register the Host Control and Prove Host Access.
    let host = HostFixture::new().await;
    let registration = harmos_runtime::operations::Registration::new(harmos_runtime::operations::Lane::default())
        .entry(OperationFactory::protobuf::<Control>().unwrap());
    host.runtime.register_batch(registration).await.unwrap();
    let response = host.runtime.operations.call_protobuf::<Control>(Default::default()).await.unwrap();
    assert_eq!(response.now.unwrap().seconds, 17);

    // The Caller Declares the Exact Control Contract but Cannot Acquire Host Visibility.
    let caller = host.sdk("caller").await;
    let _caller = host.runtime.attach_grpc(Placement::Remote(caller.address.clone()), support::settings()).await.unwrap();
    let error = host.runtime.operations.call_encoded("fixture.forward", payload(b"control")).await.unwrap_err();
    assert_eq!(call_error(&error).classification(), harmos_protocol::v1::Classification::Refused);
    assert!(call_error(&error).to_string().contains("fixture.control"));
    host.runtime.shutdown().await.unwrap();
}
