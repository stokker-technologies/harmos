/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Detach Only the Host's Side of an Unmanaged Plugin

/* === Imports === */

// Standard Library Imports
use std::{fs, process::Stdio};

// Third Party Imports
use tokio::process::Command;

// Local Imports
use super::manager::support::{Backends, Host};
use super::support::Remote;
use super::{manager, support};
use harmos_protocol::{
    host::{PluginState, PluginValue, PluginsDeleteRequest, PluginsReplaceRequest, PluginsSetRequest, PluginsWatchRequest},
    plugin::{InstanceGetRequest, InstanceStopRequest, instance_client::InstanceClient},
};
use harmos_runtime::plugin::grpc::{Launches, Placement};

/* === Tests === */

#[tokio::test]
async fn unmanaged_removal_and_disable_preserve_the_server_for_registration() {
    // Register an Independently Serving Plugin.
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let remote = Remote::start().await;
    let mut watch = host.client.watch(PluginsWatchRequest::default()).await.unwrap().into_inner();
    let request = manager::support::remote("remote", &remote.address);
    host.client.create(request).await.unwrap();
    let ready = manager::support::state(&mut watch, "remote", PluginState::Ready, 1).await;
    let first = host.manager.activation("remote").await.unwrap();

    // Registry Replacement Still Requires a Managed Entry and Leaves This Activation Alone.
    let request = PluginsReplaceRequest {
        name: ready.name.clone(),
        placement: manager::support::remote("remote", &remote.address).placement,
        expected_revision: ready.revision.clone(),
        request_id: "replace-remote".into(),
    };
    assert!(host.client.replace(request).await.is_err());
    assert!(first.is_connected());
    assert_eq!(remote.stops(), 0);

    // Disable Without Sending Stop, Then Admit the Same Endpoint Again.
    let request = PluginsSetRequest {
        name: ready.name,
        value: Some(PluginValue { enabled: false }),
        expected_revision: ready.revision,
    };
    let disabled = host.client.set(request).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(disabled.state, PluginState::Stopped as i32);
    assert!(!first.is_connected());
    assert!(first.get().await.is_err());
    assert_eq!(remote.stops(), 0);
    assert_eq!(remote.get().await.id, "fixture");
    let request = PluginsSetRequest {
        name: disabled.name,
        value: Some(PluginValue { enabled: true }),
        expected_revision: disabled.revision,
    };
    let enabled = host.client.set(request).await.unwrap().into_inner().entry.unwrap();
    let ready = manager::support::state(&mut watch, "remote", PluginState::Ready, enabled.activation).await;
    let second = host.manager.activation("remote").await.unwrap();
    assert!(second.is_connected());
    assert_eq!(second.description.started_at, first.description.started_at);

    // Remove the Entry, Then Register the Same Process Under a Fresh Entry.
    let request = PluginsDeleteRequest {
        name: ready.name,
        expected_revision: ready.revision,
    };
    host.client.delete(request).await.unwrap();
    assert!(!second.is_connected());
    assert!(host.manager.activation("remote").await.is_err());
    assert_eq!(remote.stops(), 0);
    assert_eq!(remote.get().await.id, "fixture");
    let mut request = manager::support::remote("remote", &remote.address);
    request.request_id = "remote-again".into();
    host.client.create(request).await.unwrap();
    manager::support::state(&mut watch, "remote", PluginState::Ready, 1).await;
    assert!(host.manager.activation("remote").await.unwrap().is_connected());
    drop(watch);
    host.close().await;
    assert_eq!(remote.stops(), 0);
    assert_eq!(remote.get().await.id, "fixture");
}

#[tokio::test]
async fn replacing_an_unmanaged_attachment_preserves_both_servers() {
    // Attach the Original Endpoint.
    let runtime = support::runtime(None).await;
    let original = Remote::start().await;
    let replacement = Remote::start().await;
    let attached = runtime.attach_grpc(Placement::Remote(original.address.clone()), support::settings()).await.unwrap();
    let activation = attached.activation().unwrap();

    // Replace the Attachment Without Owning Either Process.
    attached.stop("Replaced").await.unwrap();
    let next = runtime.attach_grpc(Placement::Remote(replacement.address.clone()), support::settings()).await.unwrap();
    assert!(!activation.is_connected());
    assert!(next.activation().unwrap().is_connected());
    assert_eq!(original.stops(), 0);
    assert_eq!(replacement.stops(), 0);
    assert_eq!(original.get().await.id, "fixture");
    assert_eq!(replacement.get().await.id, "fixture");

    // The Original Process Can Register Again After Both Attachments End.
    next.stop("Complete").await.unwrap();
    let again = runtime.attach_grpc(Placement::Remote(original.address.clone()), support::settings()).await.unwrap();
    assert!(again.activation().unwrap().is_connected());
    runtime.stop().await.unwrap();
    assert_eq!(original.stops(), 0);
    assert_eq!(replacement.stops(), 0);
}

#[tokio::test]
async fn credentialed_external_run_survives_removal_and_registers_again() {
    // A Developer Tool Spawns the Rust Run Entry With Launch Credentials.
    let root = tempfile::tempdir().unwrap();
    let cleaned = root.path().join("cleanup");
    let launches = Launches::new();
    let mut launch = launches.reserve("placement").await.unwrap();
    let mut child = Command::new(support::binary("harmos-sdk-host-fixture"))
        .arg(&cleaned)
        .envs(launch.environment())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let address = format!("http://{}", launch.announced().await.unwrap());
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let mut watch = host.client.watch(PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(manager::support::remote("placement", &address)).await.unwrap();
    let ready = manager::support::state(&mut watch, "placement", PluginState::Ready, 1).await;
    assert!(ready.process.is_none());
    let activation = host.manager.activation("placement").await.unwrap();

    // Registry Removal Ends the Host Connection, Without Running Process Cleanup.
    let request = PluginsDeleteRequest {
        name: ready.name,
        expected_revision: ready.revision,
    };
    host.client.delete(request).await.unwrap();
    assert!(!activation.is_connected());
    assert!(child.try_wait().unwrap().is_none());
    assert!(!cleaned.exists());
    let mut client = InstanceClient::connect(address.clone()).await.unwrap();
    let description = client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await.unwrap().into_inner();
    assert_eq!(description.started_at, activation.description.started_at);

    // The Same Run Entry Survives Another Registration and Host Shutdown.
    let mut request = manager::support::remote("placement", &address);
    request.request_id = "placement-again".into();
    host.client.create(request).await.unwrap();
    manager::support::state(&mut watch, "placement", PluginState::Ready, 1).await;
    drop(watch);
    host.close().await;
    assert!(child.try_wait().unwrap().is_none());
    assert!(!cleaned.exists());
    assert!(client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await.is_ok());

    // Only the External Owner Requests Process Exit and Reaps It.
    client.stop(InstanceStopRequest { reason: "Owner Finished".into() }).await.unwrap();
    assert!(child.wait().await.unwrap().success());
    assert_eq!(fs::read(cleaned).unwrap(), b"stopped");
    launches.shutdown().await;
}
