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
use harmos_runtime::plugin::grpc::{Managed, Placement};

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
    assert_eq!(activation.describe().await.unwrap(), activation.description);

    // Host Shutdown Runs Stop Hooks And Reaps The Process
    runtime.shutdown().await.unwrap();
    attached.stop("Already Stopped").await.unwrap();
    assert_eq!(std::fs::read(cleaned).unwrap(), b"stopped");
    assert!(!activation.is_connected());
    assert!(activation.describe().await.is_err());
}
