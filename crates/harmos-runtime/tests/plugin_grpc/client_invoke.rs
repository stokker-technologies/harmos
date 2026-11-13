/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Client Calls a Public Plugin Through the Host

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::*, support};
use harmos_runtime::plugin::grpc::Placement;
use std::{num::NonZeroUsize, time::Duration};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn client_invokes_public_plugin_through_the_host_and_protected_is_refused() {
    // Admit a Plugin and the Generic Client Service on the Same Host.
    let host = HostFixture::new().await;
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let control = harmos_operations::execution::operations::OperationFactory::protobuf::<super::protected_builtin::Control>().unwrap();
    let control_contract = control.contract().unwrap();
    host.runtime.register_batch(harmos_runtime::operations::Registration::new(Default::default()).entry(control)).await.unwrap();
    let plugin = host.sdk("provider").await;
    let plugin = host.runtime.attach_grpc(Placement::Remote(plugin.address.clone()), support::settings()).await.unwrap();
    let listener = host
        .runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            operations: Default::default(),
            reflection: false,
            address: "127.0.0.1:0".parse().unwrap(),
            tls: None,
            connections: NonZeroUsize::new(4).unwrap(),
            calls_per_connection: NonZeroUsize::new(8).unwrap(),
            request_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(2),
        })
        .await
        .unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut client = harmos_protocol::v1::invocations_client::InvocationsClient::new(channel.clone());

    let activation = plugin.activation().unwrap();

    // Public Operations Are Invoked Through the Host Registry.
    let operation = activation
        .description
        .catalog
        .as_ref()
        .unwrap()
        .operations
        .iter()
        .find(|operation| operation.id == "example.plugin.v1.Steps/Execute")
        .unwrap();
    let input = payload(b"client request");
    let response = client
        .invoke(harmos_protocol::v1::InvocationsInvokeRequest {
            operation: operation.id.clone(),
            fingerprint: operation.fingerprint.clone(),
            payload: input.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.payload, input);

    // Neither a Matching Fingerprint nor the Client Carrier Broadens Protected Visibility.
    let operation = activation
        .description
        .catalog
        .as_ref()
        .unwrap()
        .operations
        .iter()
        .find(|operation| operation.id == "example.plugin.v1.Lifecycle/Initialize")
        .unwrap();
    let error = client
        .invoke(harmos_protocol::v1::InvocationsInvokeRequest {
            operation: operation.id.clone(),
            fingerprint: operation.fingerprint.clone(),
            payload: payload(b"refused"),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("example.plugin.v1.Lifecycle/Initialize"));
    // The Same Client Cannot Reach a Protected Host Built-In.
    let error = client
        .invoke(harmos_protocol::v1::InvocationsInvokeRequest {
            operation: "fixture.control".into(),
            fingerprint: control_contract.fingerprint().0.to_vec(),
            payload: Vec::new(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("fixture.control"));

    // The Client Listener Does Not Serve Either of the Other Faces.
    let mut instance = harmos_protocol::plugin::instance_client::InstanceClient::new(channel.clone());
    let error = instance.get(harmos_protocol::plugin::InstanceGetRequest { revision: 1, ..Default::default() }).await.unwrap_err();
    assert_eq!(error.code(), tonic::Code::Unimplemented);
    let mut context = tonic::client::Grpc::new(channel);
    context.ready().await.unwrap();
    let result: Result<tonic::Response<harmos_protocol::context::TimeGetResponse>, tonic::Status> = context
        .unary(
            tonic::Request::new(harmos_protocol::context::TimeGetRequest {}),
            tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.context.v1.Time/Get"),
            tonic_prost::ProstCodec::default(),
        )
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unimplemented);
    drop(listener);
    host.runtime.shutdown().await.unwrap();
}
