/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Structured Plugin Failure

/* === Imports === */

use {harmos_protocol::model, harmos_protocol::model::Classification};

use prost::Message;

#[derive(prost::Message)]
struct Report {
    #[prost(bytes = "vec", tag = "1")]
    value: Vec<u8>,
}

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
    let failure = model::Failure::from(failure.as_ref());
    assert_eq!(failure.classification(), Classification::Permanent);
    assert_eq!(failure.code, "fixture_failure");
    assert_eq!(failure.operation, "fixture.failure");
    assert_eq!(failure.plugin, "fixture");
    assert_eq!(failure.details.get("fixture").unwrap(), "value");
    assert_eq!(failure.diagnostics, "Fixture Diagnostics");
    let cause = failure.cause.as_ref().unwrap();
    assert_eq!(cause.classification(), Classification::Transient);
    assert_eq!(cause.code, "fixture_cause");

    // A Failed Invocation Does Not Close Other Calls
    let input = super::invoke_support::payload(b"after failure");
    assert_eq!(host.runtime.operations.call_encoded("fixture.echo", input.clone()).await.unwrap(), input);
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}

#[tokio::test]
async fn error_kinds_retry_evidence_and_failure_mapping_cross_the_host() {
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock)).await;
    let sdk = host.sdk("error-kinds").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();
    for kind in ["transient", "permanent", "refused", "invalid", "cancelled", "mapped", "unmapped"] {
        let call = host.runtime.operations.submit_encoded("fixture.error_report", super::invoke_support::payload(kind.as_bytes())).unwrap();
        let id = call.id();
        let advance = controller.advance_until(std::time::Duration::from_secs(10), || host.runtime.operations.status(id).is_none());
        let (answer, advanced) = tokio::join!(call, advance);
        advanced.unwrap();
        let answer = Report::decode(answer.unwrap().as_slice()).unwrap().value;
        let value: serde_json::Value = serde_json::from_slice(&answer).unwrap();
        assert_eq!(value["operation"], "fixture.private");
        assert_eq!(value["retry"]["attempts"], if kind == "transient" { 3 } else { 1 });
        assert_eq!(value["retry"]["max_attempts"], 3);
        assert_eq!(value["retry"]["retryable"], false);
        assert_eq!(value["retry"]["next_after_s"], 0.0);
        match kind {
            "mapped" => {
                assert_eq!(value["kind"], "refused");
                assert_eq!(value["action"], "Select a Source");
            }
            "unmapped" => {
                assert_eq!(value["kind"], "permanent");
                assert_eq!(value["message"], "Unmapped Failure");
            }
            _ => {
                assert_eq!(value["kind"], kind);
                assert_eq!(value["action"], "Choose Another Input");
            }
        }
    }
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
