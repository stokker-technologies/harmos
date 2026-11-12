/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Malformed Requests Leave the Connection Available

/* === Tests === */

#[tokio::test]
async fn malformed_payload_fails_only_its_invocation() {
    // Connect An SDK Plugin
    let host = super::invoke_support::HostFixture::new().await;
    let sdk = host.sdk("caller").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Refuse Malformed Protobuf Without Running A Handler
    let error = host.runtime.operations.call_encoded("fixture.echo", vec![0xff]).await.unwrap_err();
    assert!(matches!(super::invoke_support::call_error(&error), harmos_runtime::plugin::grpc::CallError::Transport(_)));

    // Binary And Empty Payloads Remain Valid On The Same Connection
    let input = super::invoke_support::payload(&[0, 0xff, 1, 0x80]);
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", Vec::new()).await.unwrap(), Vec::<u8>::new());
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
