/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Client Manages a Remote Plugin

/* === Imports === */

use super::support::{self, Backends, Host, Peer};
use crate::plugin_grpc::invoke_support::payload;
use harmos_protocol::v1 as wire;
use harmos_runtime::plugin::manager::Management;

/* === Tests === */

#[tokio::test]
async fn client_manages_remote_plugins() {
    // Start the Host and an Independently Owned Reference Plugin
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let mut peer = Peer::start().await;
    // Apply the Product's Existing Authorization Policy
    host.policy.0.store(false, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(host.client.list(support::list()).await.unwrap_err().code(), tonic::Code::PermissionDenied);
    host.policy.0.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut watch = host.client.watch(harmos_protocol::client::PluginsWatchRequest::default()).await.unwrap().into_inner();

    // Create a Remote Entry and Observe Its Admitted Catalog
    let request = support::remote("remote", &peer.url);
    let created = host.client.create(request.clone()).await.unwrap().into_inner();
    assert!(created.receipt.as_ref().unwrap().completed);
    let ready = support::state(&mut watch, "remote", harmos_protocol::client::PluginState::Ready, 1).await;
    let description = ready.description.as_ref().unwrap();
    assert_eq!(description.id, peer.description.id);
    assert_eq!(description.version, peer.description.version);
    assert_eq!(description.catalog, peer.description.catalog);
    assert!(!ready.catalog_digest.is_empty());
    assert!(ready.process.is_none());
    assert_eq!(host.client.create(request.clone()).await.unwrap().into_inner(), created);
    let mut changed = request;
    changed.value.as_mut().unwrap().enabled = false;
    assert!(host.client.create(changed).await.is_err());

    // Read the Complete Entry at One Resolved Position
    let got = host.client.get(harmos_protocol::client::PluginsGetRequest { name: "remote".into(), at: None }).await.unwrap().into_inner();
    let mut page = support::list();
    page.at = got.resolved_at.clone();
    let listed = host.client.list(page).await.unwrap().into_inner();
    assert_eq!(listed.entries, vec![ready.clone()]);
    assert_eq!(listed.resolved_at, got.resolved_at);

    // Call the Same Contract Through the Rust SDK Client
    let factory = harmos_runtime::operations::OperationFactory::protobuf::<harmos_operations::plugins::Get>().unwrap();
    let contract = factory.contract().unwrap();
    let binding = contract.grpc().unwrap();
    let declaration = wire::DiscoveredContract {
        id: contract.operation().id.to_string(),
        visibility: wire::Visibility::Public as i32,
        protobuf: contract.protobuf().cloned(),
        fingerprint: contract.fingerprint().0.to_vec(),
        grpc: Some(wire::Grpc {
            package: binding.package.to_string(),
            service: binding.service.to_string(),
            method: binding.method.to_string(),
            managed: binding.managed,
        }),
        ..Default::default()
    };
    let method =
        harmos::client::Method::<harmos_protocol::client::PluginsGetRequest, harmos_protocol::client::PluginsGetResponse>::new(declaration)
            .unwrap();
    let capacity = std::num::NonZeroUsize::new(1024 * 1024).unwrap();
    let sdk = harmos::client::Client::new(
        host.channel.clone(),
        harmos::client::ClientSettings {
            transfer: harmos_protocol::v1::StorageTransferSettings { chunk_bytes: Some(512 * 1024) },
            retained_bytes: capacity,
            message_bytes: capacity,
            input_items: std::num::NonZeroUsize::new(1).unwrap(),
            input_bytes: capacity,
        },
    )
    .unwrap();
    let sdk_response = sdk
        .call(
            &method,
            tonic::Request::new(harmos_protocol::client::PluginsGetRequest {
                name: "remote".into(),
                at: got.resolved_at,
            }),
        )
        .await
        .unwrap();
    assert_eq!(sdk_response.into_inner().entry, Some(ready.clone()));

    // Invoke Through the Existing Scoped Host Dispatcher
    let operations = host.runtime.operations.with_scope("opaque-unit").unwrap();
    let response = operations.call_encoded("conformance.step.0", payload(b"scope")).await.unwrap();
    assert_eq!(response, payload(b"opaque-unit"));

    // Disable Using the Whole Writable Value and Refuse Its Stale Revision
    let disabled = host
        .client
        .set(harmos_protocol::client::PluginsSetRequest {
            name: "remote".into(),
            value: Some(harmos_protocol::client::PluginValue { enabled: false }),
            expected_revision: ready.revision.clone(),
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    assert_eq!(disabled.state, harmos_protocol::client::PluginState::Stopped as i32);
    assert!(host.runtime.operations.call_encoded("conformance.step.0", payload(b"disabled")).await.is_err());
    assert!(
        host.client
            .set(harmos_protocol::client::PluginsSetRequest {
                name: "remote".into(),
                value: Some(harmos_protocol::client::PluginValue { enabled: true }),
                expected_revision: ready.revision
            })
            .await
            .is_err()
    );

    // Enable and Restart With Fresh Activation Evidence
    let enabled = host
        .client
        .set(harmos_protocol::client::PluginsSetRequest {
            name: "remote".into(),
            value: Some(harmos_protocol::client::PluginValue { enabled: true }),
            expected_revision: disabled.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    let ready = support::state(&mut watch, "remote", harmos_protocol::client::PluginState::Ready, enabled.activation).await;
    let restarting = host
        .client
        .restart(harmos_protocol::client::PluginsRestartRequest {
            name: "remote".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    let ready = support::state(&mut watch, "remote", harmos_protocol::client::PluginState::Ready, restarting.activation).await;
    assert!(ready.activation > enabled.activation);

    // List The Complete Configured Set And Repeat Its Issued Cut
    let mut other = support::remote("other", &peer.url);
    other.value.as_mut().unwrap().enabled = false;
    let mut other = host.client.create(other).await.unwrap().into_inner().entry.unwrap();
    let first = host.client.list(support::list()).await.unwrap().into_inner();
    assert_eq!(first.entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(), ["other", "remote"]);
    let repeated = host.client.list(harmos_protocol::client::PluginsListRequest { at: first.resolved_at }).await.unwrap().into_inner();
    assert_eq!(repeated.entries, first.entries);

    // Report an Explicit Gap to an Observer That Retains Its Bounded Queue
    let mut slow = host.manager.watch(harmos_protocol::client::PluginsWatchRequest::default()).await.unwrap();
    let capacity = support::settings(root.path()).watch_capacity.get();
    for _ in 0..(capacity * 2 + 2) {
        let enabled = !other.value.as_ref().unwrap().enabled;
        let request = harmos_protocol::client::PluginsSetRequest {
            name: other.name.clone(),
            value: Some(harmos_protocol::client::PluginValue { enabled }),
            expected_revision: other.revision,
        };
        other = host.client.set(request).await.unwrap().into_inner().entry.unwrap();
    }
    loop {
        let event = slow.recv().await.unwrap();
        if matches!(event.event, Some(harmos_protocol::client::plugins_watch_response::Event::ResyncRequired(_))) {
            break;
        }
    }
    host.client
        .delete(harmos_protocol::client::PluginsDeleteRequest {
            name: "other".into(),
            expected_revision: other.revision,
        })
        .await
        .unwrap();

    // Delete the Entry Without Terminating Its External Process
    host.client
        .delete(harmos_protocol::client::PluginsDeleteRequest {
            name: "remote".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    assert!(host.client.list(support::list()).await.unwrap().into_inner().entries.is_empty());
    assert!(peer.is_running());
    assert!(host.runtime.operations.definition("conformance.step.0").is_err());
    drop(watch);
    host.close().await;
    peer.close().await;
}
