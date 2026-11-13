/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Plugin Environment Story

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{invoke_support::payload, support};
use harmos_runtime::{
    clock::Clock,
    plugin::grpc::{Managed, Placement},
};

/* === Tests === */

#[tokio::test]
async fn managed_plugin_receives_launch_environment() {
    // Launch An SDK Plugin With Consumer-Owned Unicode Values
    let (clock, _) = Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    let runtime = support::runtime(Some(clock)).await;
    let evidence = tempfile::tempdir().unwrap();
    let cleaned = evidence.path().join("cleanup");
    let value = evidence.path().join("Plugin Data é").to_str().unwrap().to_owned();
    let placement = Placement::Managed(Managed {
        executable: support::binary("harmos-sdk-host-fixture"),
        arguments: vec![cleaned.clone().into_os_string()],
        environment: [("HARMOS_EXPLICIT".into(), value.clone().into())].into(),
    });
    let attached = runtime.attach_grpc(placement, support::settings()).await.unwrap();

    // Read The Launch Value Through A Protobuf Operation
    let response = runtime.operations.call_encoded("placement.environment", payload(b"HARMOS_EXPLICIT")).await.unwrap();
    assert_eq!(response, payload(value.as_bytes()));

    // Preserve Windows System Paths In The Managed Child
    #[cfg(windows)]
    for name in ["SystemRoot", "ProgramData"] {
        let inherited = std::env::var(name).unwrap();
        let response = runtime.operations.call_encoded("placement.environment", payload(name.as_bytes())).await.unwrap();
        assert_eq!(response, payload(inherited.as_bytes()));
    }

    // Stop Reaps The Child After Its Cleanup Hook
    runtime.shutdown().await.unwrap();
    assert!(!attached.activation().is_some_and(|activation| activation.is_connected()));
    assert_eq!(std::fs::read(cleaned).unwrap(), b"stopped");
}
