/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Host Restores Plugin Intent After Restart

/* === Imports === */

use super::support::{self, Backends, Host};
use crate::plugin_grpc::support::Remote;
use harmos_protocol::host as wire;
use harmos_runtime::plugin::manager::Management;

/* === Tests === */

#[tokio::test]
async fn host_restores_plugin_entries() {
    // Persist Remote, Enabled Managed and Disabled Managed Entries
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let remote = Remote::start().await;
    let mut host = Host::start(root.path(), &backends).await;
    let content = host.upload("bundle", support::bundle(true)).await;
    let request = support::managed("enabled", content.clone(), true);
    let original = host.client.create(request.clone()).await.unwrap().into_inner();
    host.client.create(support::managed("disabled", content, false)).await.unwrap();
    host.client.create(support::remote("remote", &remote.address)).await.unwrap();
    let mut watch = host.client.watch(wire::PluginsWatchRequest::default()).await.unwrap().into_inner();
    let managed = support::state(&mut watch, "enabled", wire::PluginState::Ready, 1).await;
    let before = host.client.list(support::page()).await.unwrap().into_inner();
    let old_cut = before.resolved_at;
    drop(watch);
    let first_id = managed.process.as_ref().unwrap().id;
    let first = support::Process::observe(first_id).await;
    host.close().await;
    assert!(first.exited().await.success());

    // Restore Durable Entries and Reconstruct Live State
    let mut host = Host::start(root.path(), &backends).await;
    let mut watch = host.client.watch(wire::PluginsWatchRequest::default()).await.unwrap().into_inner();
    let managed_after = support::state(&mut watch, "enabled", wire::PluginState::Ready, managed.activation + 1).await;
    let remote_after = host.manager.get(wire::PluginsGetRequest { name: "remote".into(), at: None }).await.unwrap();
    assert!(remote_after.entry.unwrap().placement.is_some());
    let remote_watch = host.client.watch(wire::PluginsWatchRequest { name: "remote".into(), at: None }).await.unwrap();
    let mut remote_watch = remote_watch.into_inner();
    let remote_after = support::state(&mut remote_watch, "remote", wire::PluginState::Ready, 2).await;
    assert_eq!(managed_after.name, managed.name);
    assert_eq!(managed_after.placement, managed.placement);
    assert_eq!(managed_after.revision, managed.revision);
    assert!(managed_after.process.is_some());
    let restored = support::Process::observe(managed_after.process.as_ref().unwrap().id).await;
    assert!(remote_after.process.is_none());
    let disabled =
        host.client.get(wire::PluginsGetRequest { name: "disabled".into(), at: None }).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(disabled.state, wire::PluginState::Stopped as i32);
    assert!(disabled.process.is_none());
    assert_eq!(host.client.create(request).await.unwrap().into_inner(), original);
    assert!(host.client.get(wire::PluginsGetRequest { name: "enabled".into(), at: old_cut }).await.is_err());

    // Settle Both Restored Activations With the Host
    drop(watch);
    drop(remote_watch);
    host.close().await;
    assert!(restored.exited().await.success());
}
