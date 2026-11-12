/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Structured Plugin Failure

/* === Tests === */

#[tokio::test]
async fn structured_failure_preserves_causes_and_leaves_the_connection_usable() {
    // Connect An SDK Plugin
    let host = super::invoke_support::HostFixture::new().await;
    let sdk = host.sdk("caller").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Read The Application Failure Through The Runtime
    let error = host.runtime.operations.call_encoded("fixture.failure", Vec::new()).await.unwrap_err();
    let harmos_runtime::plugin::grpc::CallError::Remote(failure) = super::invoke_support::call_error(&error) else {
        panic!("Expected An Application Failure");
    };
    assert_eq!(failure.classification(), harmos_protocol::v1::Classification::Permanent);
    assert_eq!(failure.code, "fixture_failure");
    assert_eq!(failure.operation, "fixture.failure");
    assert_eq!(failure.plugin, "caller");
    assert_eq!(failure.details.get("fixture").unwrap(), "value");
    assert_eq!(failure.diagnostics, "Fixture Diagnostics");
    let cause = failure.cause.as_ref().unwrap();
    assert_eq!(cause.classification(), harmos_protocol::v1::Classification::Transient);
    assert_eq!(cause.code, "fixture_cause");

    // A Failed Invocation Does Not Close Other Calls
    let input = super::invoke_support::payload(b"after failure");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
