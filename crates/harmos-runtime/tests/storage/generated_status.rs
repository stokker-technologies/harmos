/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Observe Storage Frontiers Through Generated Messages.

/* === Tests === */

#[tokio::test]
async fn reports_persisted_frontiers() {
    use harmos_operations::storage::Info;
    use harmos_protocol::storage;
    let fixture = super::Fixture::new(super::config()).await;
    crate::support::app::register(&fixture.runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
        .await
        .unwrap();
    let accepted = super::write(&fixture.storage, "protobuf-status").await;
    fixture.storage.flush().await.unwrap();
    let status = fixture.runtime.operations.call_protobuf::<Info>(storage::StorageInfoRequest {}).await.unwrap();
    assert_eq!(status.accepted, Some(accepted.clone().into()));
    assert_eq!(status.saved, Some(accepted.into()));
    let plugin = super::transfer_plugin::TransferPlugin::start().await;
    let attached = fixture
        .runtime
        .attach_grpc(
            harmos_runtime::plugin::grpc::Placement::Remote(plugin.address.clone()),
            harmos_runtime::plugin::grpc::PlacementSettings {
                connect_timeout: std::time::Duration::from_secs(30),
                reconnect_delay: std::time::Duration::from_secs(1),
                stop_grace: std::time::Duration::from_secs(30),
                stream_window: std::num::NonZeroUsize::new(64).unwrap(),
            },
        )
        .await
        .unwrap();
    for method in ["info", "get_schema", "set_schema", "delete_file"] {
        let error = fixture.runtime.operations.call_encoded(&format!("fixture.transfer.native_{method}"), Vec::new()).await.unwrap_err();
        let harmos_runtime::Error::OperationFailed { source, .. } = error else { panic!("unexpected refusal") };
        let call = source.downcast_ref::<harmos_runtime::plugin::grpc::CallError>().expect("plugin refusal");
        assert_eq!(call.classification(), harmos_protocol::v1::Classification::Refused);
    }
    let listener = fixture.runtime.grpc().attach(harmos_runtime::grpc::Settings::default()).await.unwrap();
    let channel = tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut client = tonic::client::Grpc::new(channel);
    client.ready().await.unwrap();
    let result: Result<tonic::Response<storage::StorageInfoResponse>, tonic::Status> = client
        .unary(
            tonic::Request::new(storage::StorageInfoRequest {}),
            tonic::codegen::http::uri::PathAndQuery::from_static("/harmos.storage.v1.Storage/Info"),
            tonic_prost::ProstCodec::default(),
        )
        .await;
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unimplemented);
    listener.detach().await.unwrap();
    attached.stop("native boundary complete").await.unwrap();
    plugin.stop().await;
    fixture.finish().await;
}
