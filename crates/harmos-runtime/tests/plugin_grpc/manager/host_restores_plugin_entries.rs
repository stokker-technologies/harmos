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

use harmos_protocol::host::PluginState;
use harmos_protocol::host::PluginsGetRequest;
use harmos_protocol::host::PluginsWatchRequest;

use super::support::{self, Backends, Host};
use crate::plugin_grpc::support::Remote;
use harmos_runtime::plugin::manager::Management;

/* === Tests === */

#[tokio::test]
async fn host_restores_plugin_entries() {
    // Persist Remote, Enabled Managed and Disabled Managed Entries
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let remote = Remote::start().await;
    let mut host = Host::start(root.path(), &backends).await;
    // Reopen Legacy and Labelled Disabled Entries Without Writing Storage
    let mut request = support::remote("legacy", &remote.address);
    request.value.as_mut().unwrap().enabled = false;
    host.client.create(request).await.unwrap();
    let mut request = support::remote("labelled", &remote.address);
    request.value.as_mut().unwrap().enabled = false;
    request.origin = Some("installation".into());
    host.client.create(request).await.unwrap();
    host.close().await;
    let writes = backends.data.control.writes.lock().unwrap().len();
    let recovery_writes = backends.recovery.control.writes.lock().unwrap().len();
    let mut host = Host::start(root.path(), &backends).await;
    let entries = host.client.list(support::list()).await.unwrap().into_inner().entries;
    assert_eq!(entries.iter().find(|entry| entry.name == "legacy").unwrap().origin, "");
    assert_eq!(entries.iter().find(|entry| entry.name == "labelled").unwrap().origin, "installation");
    host.close().await;
    assert_eq!(backends.data.control.writes.lock().unwrap().len(), writes);
    assert_eq!(backends.recovery.control.writes.lock().unwrap().len(), recovery_writes);

    // Persist Enabled Entries With an Origin
    let mut host = Host::start(root.path(), &backends).await;
    let content = host.upload("bundle", support::bundle(true)).await;
    let mut request = support::managed("enabled", content.clone(), true);
    request.origin = Some("installation".into());
    let original = host.client.create(request.clone()).await.unwrap().into_inner();
    host.client.create(support::managed("disabled", content, false)).await.unwrap();
    host.client.create(support::remote("remote", &remote.address)).await.unwrap();
    let mut watch = host.client.watch(PluginsWatchRequest::default()).await.unwrap().into_inner();
    let managed = support::state(&mut watch, "enabled", PluginState::Ready, 1).await;
    let before = host.client.list(support::list()).await.unwrap().into_inner();
    let old_cut = before.resolved_at;
    drop(watch);
    let first_id = managed.process.as_ref().unwrap().id;
    let first = support::Process::observe(first_id).await;
    host.close().await;
    assert!(first.exited().await.success());

    // Restore Durable Entries and Reconstruct Live State
    let writes = backends.data.control.writes.lock().unwrap().len();
    let mut host = Host::start(root.path(), &backends).await;
    assert!(backends.data.control.writes.lock().unwrap().len() > writes);
    let mut watch = host.client.watch(PluginsWatchRequest::default()).await.unwrap().into_inner();
    let managed_after = support::state(&mut watch, "enabled", PluginState::Ready, managed.activation + 1).await;
    let remote_after = host.manager.get(PluginsGetRequest { name: "remote".into(), at: None }).await.unwrap();
    assert!(remote_after.entry.unwrap().placement.is_some());
    let remote_watch = host.client.watch(PluginsWatchRequest { name: Some("remote".into()), at: None }).await.unwrap();
    let mut remote_watch = remote_watch.into_inner();
    let remote_after = support::state(&mut remote_watch, "remote", PluginState::Ready, 2).await;
    assert_eq!(managed_after.origin, "installation");
    assert_eq!(managed_after.name, managed.name);
    assert_eq!(managed_after.placement, managed.placement);
    assert_eq!(managed_after.revision, managed.revision);
    assert!(managed_after.process.is_some());
    let restored = support::Process::observe(managed_after.process.as_ref().unwrap().id).await;
    assert!(remote_after.process.is_none());
    let disabled = host.client.get(PluginsGetRequest { name: "disabled".into(), at: None }).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(disabled.state, PluginState::Stopped as i32);
    assert!(disabled.process.is_none());
    assert_eq!(host.client.create(request).await.unwrap().into_inner(), original);
    assert!(host.client.get(PluginsGetRequest { name: "enabled".into(), at: old_cut }).await.is_err());

    // Settle Both Restored Activations With the Host
    drop(watch);
    drop(remote_watch);
    host.close().await;
    assert!(restored.exited().await.success());
}
