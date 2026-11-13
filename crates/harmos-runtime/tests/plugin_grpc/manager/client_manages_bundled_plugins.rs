/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Client Installs and Supervises a Bundled Plugin

/* === Imports === */

use super::support::{self, Backends, Host};
use harmos_protocol::v1 as wire;

/* === Tests === */

#[tokio::test]
async fn client_manages_bundled_plugins() {
    // Upload a Bundle Through the Library's Published File Session
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let content = host.upload("bundle", support::bundle(true)).await;
    let mut watch = host.client.watch(wire::PluginsWatchRequest { name: "managed".into(), at: None }).await.unwrap().into_inner();

    // Create a Managed Entry and Observe Its Process Identity
    host.client.create(support::managed("managed", content, true)).await.unwrap();
    let ready = support::state(&mut watch, "managed", wire::PluginState::Ready, 1).await;
    let process = ready.process.as_ref().unwrap();
    let first_id = process.id;
    let first = support::Process::observe(first_id).await;
    assert!(!process.executable_digest.is_empty());
    assert!(process.started_at.is_some());

    // Kill Only That Process and Observe the Retained Failure
    assert!(first.kill().await.success());
    let unavailable = support::state(&mut watch, "managed", wire::PluginState::Unavailable, ready.activation).await;
    assert_eq!(unavailable.name, ready.name);
    assert_eq!(unavailable.placement, ready.placement);
    assert!(unavailable.process.is_none());
    let failure = unavailable.failure.unwrap();
    assert!(!failure.message.is_empty());
    assert!(!failure.action.is_empty());

    // Restart Replaces the Activation and Starts a New Process
    let restarting = host
        .client
        .restart(wire::PluginsRestartRequest {
            name: "managed".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    let ready = support::state(&mut watch, "managed", wire::PluginState::Ready, restarting.activation).await;
    assert!(ready.activation > unavailable.activation);
    let second_id = ready.process.unwrap().id;
    let second = support::Process::observe(second_id).await;

    // Delete Awaits Process Exit and Removes Its Installed Files
    host.client
        .delete(wire::PluginsDeleteRequest {
            name: "managed".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    assert!(second.exited().await.success());
    assert_eq!(std::fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    assert!(host.client.list(support::list()).await.unwrap().into_inner().entries.is_empty());
    drop(watch);
    host.close().await;
}
