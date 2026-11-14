/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Remote Python Plugin Waits Through the Real Host

/* === Tests === */

#[tokio::test]
async fn python_plugin_sleep_and_now_share_the_virtual_host_clock() {
    // Admit The Full Python Conformance Catalog To The Real Host
    let (clock, controller) = harmos_runtime::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let host = super::invoke_support::HostFixture::with_clock(Some(clock.clone())).await;
    let sdk = super::invoke_support::Python::start(&[]).await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Invoke Python Through The Host With Its Full Stream Inventory Active
    let payload = super::invoke_support::payload(b"python");
    let reply = host.runtime.operations.submit_encoded("conformance.step.0", payload.clone()).unwrap().await.unwrap();
    assert_eq!(reply, payload);
    // A Valid Private Handler Is Local to the SDK and Never Admitted by the Host.
    assert!(
        !attached
            .activation()
            .unwrap()
            .description
            .declarations
            .as_ref()
            .unwrap()
            .operations
            .iter()
            .any(|operation| operation.id == "conformance.local")
    );
    assert!(host.runtime.operations.submit_encoded("conformance.local", payload.clone()).is_err());
    let reply = host.runtime.operations.call_encoded("conformance.local_call", payload.clone()).await.unwrap();
    assert_eq!(reply, payload);

    // Close External Device Demand Before The Virtual Clock Journey
    attached.stop("Clock Journey").await.unwrap();
    assert!(sdk.close().await.success());
    let sdk = super::invoke_support::Python::start(&["--clock-only"]).await;
    let attached = host
        .runtime
        .attach_grpc(harmos_runtime::plugin::grpc::Placement::Remote(sdk.address.clone()), super::support::settings())
        .await
        .unwrap();

    // Advance Only Through The Plugin's Host Sleep
    let call = host.runtime.operations.submit_encoded("conformance.clock", super::invoke_support::payload(b"clock")).unwrap();
    let id = call.id();
    let drive = controller.advance_until(std::time::Duration::from_secs(7200), || host.runtime.operations.status(id).is_none());
    let (result, advanced) = tokio::join!(call, drive);
    advanced.unwrap();

    // The Two Readings Bracket Exactly One Hour
    assert_eq!(clock.date(), std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(3600));
    assert_eq!(result.unwrap(), super::invoke_support::payload(&3600_i64.to_le_bytes()));
    attached.stop("Story Complete").await.unwrap();
    host.runtime.shutdown().await.unwrap();
    assert!(sdk.close().await.success());
}
