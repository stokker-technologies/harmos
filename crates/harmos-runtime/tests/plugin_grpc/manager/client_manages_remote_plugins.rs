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
    assert_eq!(host.client.list(support::page()).await.unwrap_err().code(), tonic::Code::PermissionDenied);
    host.policy.0.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut watch = host.client.watch(wire::PluginsWatchRequest::default()).await.unwrap().into_inner();

    // Create a Remote Entry and Observe Its Admitted Catalog
    let request = support::remote("remote", &peer.url);
    let created = host.client.create(request.clone()).await.unwrap().into_inner();
    assert!(created.receipt.as_ref().unwrap().completed);
    let ready = support::state(&mut watch, "remote", wire::PluginState::Ready, 1).await;
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
    let got = host.client.get(wire::PluginsGetRequest { name: "remote".into(), at: None }).await.unwrap().into_inner();
    let mut page = support::page();
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
    let method = harmos::client::Method::<wire::PluginsGetRequest, wire::PluginsGetResponse>::new(declaration).unwrap();
    let capacity = std::num::NonZeroUsize::new(1024 * 1024).unwrap();
    let sdk = harmos::client::Client::new(
        host.channel.clone(),
        harmos::client::ClientSettings {
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
            tonic::Request::new(wire::PluginsGetRequest {
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
        .set(wire::PluginsSetRequest {
            name: "remote".into(),
            value: Some(wire::PluginValue { enabled: false }),
            expected_revision: ready.revision.clone(),
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    assert_eq!(disabled.state, wire::PluginState::Stopped as i32);
    assert!(host.runtime.operations.call_encoded("conformance.step.0", payload(b"disabled")).await.is_err());
    assert!(
        host.client
            .set(wire::PluginsSetRequest {
                name: "remote".into(),
                value: Some(wire::PluginValue { enabled: true }),
                expected_revision: ready.revision
            })
            .await
            .is_err()
    );

    // Enable and Restart With Fresh Activation Evidence
    let enabled = host
        .client
        .set(wire::PluginsSetRequest {
            name: "remote".into(),
            value: Some(wire::PluginValue { enabled: true }),
            expected_revision: disabled.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    let ready = support::state(&mut watch, "remote", wire::PluginState::Ready, enabled.activation).await;
    let restarting = host
        .client
        .restart(wire::PluginsRestartRequest {
            name: "remote".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    let ready = support::state(&mut watch, "remote", wire::PluginState::Ready, restarting.activation).await;
    assert!(ready.activation > enabled.activation);

    // Continue One Bound Page Without Losing an Entry
    let mut other = support::remote("other", &peer.url);
    other.value.as_mut().unwrap().enabled = false;
    let mut other = host.client.create(other).await.unwrap().into_inner().entry.unwrap();
    let mut request = support::page();
    request.page.as_mut().unwrap().page_size = 1;
    let first = host.client.list(request.clone()).await.unwrap().into_inner();
    assert_eq!(first.entries[0].name, "other");
    request.at = first.resolved_at;
    request.page.as_mut().unwrap().page_token = first.next_page_token;
    let second = host.client.list(request.clone()).await.unwrap().into_inner();
    assert_eq!(second.entries[0].name, "remote");
    assert!(second.next_page_token.is_empty());
    request.page.as_mut().unwrap().page_token.push('0');
    assert!(host.client.list(request).await.is_err());

    // Report an Explicit Gap to an Observer That Retains Its Bounded Queue
    let mut slow = host.manager.watch(wire::PluginsWatchRequest::default()).await.unwrap();
    let capacity = support::settings(root.path()).watch_capacity.get();
    for _ in 0..(capacity * 2 + 2) {
        let enabled = !other.value.as_ref().unwrap().enabled;
        let request = wire::PluginsSetRequest {
            name: other.name.clone(),
            value: Some(wire::PluginValue { enabled }),
            expected_revision: other.revision,
        };
        other = host.client.set(request).await.unwrap().into_inner().entry.unwrap();
    }
    loop {
        let event = slow.recv().await.unwrap();
        if matches!(event.event, Some(wire::plugins_watch_response::Event::ResyncRequired(_))) {
            break;
        }
    }
    host.client
        .delete(wire::PluginsDeleteRequest {
            name: "other".into(),
            expected_revision: other.revision,
        })
        .await
        .unwrap();

    // Delete the Entry Without Terminating Its External Process
    host.client
        .delete(wire::PluginsDeleteRequest {
            name: "remote".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    assert!(host.client.list(support::page()).await.unwrap().into_inner().entries.is_empty());
    assert!(peer.is_running());
    assert!(host.runtime.operations.definition("conformance.step.0").is_err());
    drop(watch);
    host.close().await;
    peer.close().await;
}
