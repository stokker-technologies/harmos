/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Operation Deadline Uses the Host Clock

/* === Tests === */

#[tokio::test]
async fn attempt_deadline_cancels_a_plugin_sleep_before_its_timer_is_due() {
    // Connect A Plugin Whose Sleep Exceeds Its Operation Budget
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = host.sdk("deadline").await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Advance The Host Until The Attempt Settles
    let call = host.runtime.operations.submit_encoded("fixture.clock", Vec::new()).unwrap();
    let id = call.id();
    let advancing = controller.advance_until(std::time::Duration::from_secs(10), || host.runtime.operations.status(id).is_none());
    let (answer, advanced) = tokio::join!(call, advancing);
    advanced.unwrap();
    assert!(matches!(answer, Err(harmos_runtime::Error::OperationRefused(harmos_runtime::Refusal::TimedOut { .. }))));
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2));

    // A Deadline Leaves The Connection Available
    assert!(attached.activation().unwrap().get().await.is_ok());
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
