/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed SDK Plugin Lifecycle

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support;
use harmos_runtime::plugin::grpc::{Error, Managed, Placement};

/* === Tests === */

#[tokio::test]
async fn managed_sdk_plugin_is_described_and_stopped_with_its_host() {
    // Launch The Host And Managed SDK Plugin
    let runtime = support::runtime(None).await;
    let evidence = tempfile::tempdir().unwrap();
    let cleaned = evidence.path().join("cleanup");
    let placement = Placement::Managed(Managed {
        executable: support::binary("harmos-sdk-host-fixture"),
        arguments: vec![cleaned.clone().into_os_string()],
        environment: Default::default(),
    });
    let attached = runtime.attach_grpc(placement, support::settings()).await.unwrap();

    // Inspect The Connection's Identity And Executable Evidence
    let activation = attached.activation().unwrap();
    assert_eq!(activation.description.id, "placement");
    assert!(activation.description.started_at.is_some());
    assert!(activation.executable_digest.is_some());
    assert_eq!(activation.get().await.unwrap(), activation.description);

    // Host Shutdown Runs Stop Hooks And Reaps The Process
    runtime.stop().await.unwrap();
    attached.stop("Already Stopped").await.unwrap();
    assert_eq!(std::fs::read(cleaned).unwrap(), b"stopped");
    assert!(!activation.is_connected());
    assert!(activation.get().await.is_err());
}

#[tokio::test]
async fn a_stopping_host_refuses_to_start_a_plugin() {
    // The Host Has Begun Shutting Down Before Anyone Attaches.
    let runtime = support::runtime(None).await;
    runtime.stop().await.unwrap();

    // Attaching Refuses With an Action Instead of Spawning a Process the Host Would Have to Reap.
    let placement = Placement::Managed(Managed {
        executable: support::binary("harmos-sdk-host-fixture"),
        arguments: vec![tempfile::tempdir().unwrap().path().join("cleanup").into_os_string()],
        environment: Default::default(),
    });
    let refused = runtime.attach_grpc(placement, support::settings()).await;
    assert!(matches!(refused, Err(Error::Invalid(_))), "Refuse to Start a Plugin on a Stopping Host: {:?}", refused.err());
}
