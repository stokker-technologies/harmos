/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Client Refuses Invalid Bundles Before Startup

/* === Imports === */

use super::support::{self, Backends, Host};
use harmos_protocol::v1 as wire;
use harmos_runtime::plugin::manager::Management;

/* === Tests === */

#[tokio::test]
async fn client_refuses_invalid_plugin_bundles() {
    // Upload a Tampered Bundle With Its Actual Transfer Digest
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let mut bytes = support::bundle(true);
    let mut archive = tar::Archive::new(bytes.as_slice());
    let offset = archive
        .entries()
        .unwrap()
        .find_map(|entry| {
            let entry = entry.unwrap();
            entry.path().unwrap().starts_with("bin").then_some(entry.raw_file_position() as usize)
        })
        .unwrap();
    bytes[offset] ^= 1;
    let content = host.upload("tampered", bytes).await;

    // Refuse Manifest Integrity Failure Before Admitting an Entry
    let error = host.manager.create(support::managed("tampered", content, true)).await.unwrap_err();
    assert!(!error.0.action.is_empty());
    assert_eq!(host.client.list(support::page()).await.unwrap().into_inner().entries.len(), 0);
    assert!(!root.path().join("plugins").exists());

    // Refuse a Bundle Whose Manifest Declares No Native Executable
    let content = host.upload("missing", support::bundle(false)).await;
    let error = host.manager.create(support::managed("missing", content, true)).await.unwrap_err();
    assert_eq!(error.0.code, "executable_missing");
    assert!(!error.0.action.is_empty());
    assert!(host.runtime.operations.definition("conformance.echo").is_err());
    assert!(host.client.list(support::page()).await.unwrap().into_inner().entries.is_empty());
    assert!(!root.path().join("plugins").exists());

    // Refuse a Claimed Transfer Digest That Does Not Match Published Content
    let mut content = host.upload("digest", support::bundle(true)).await;
    content.sha256[0] ^= 1;
    assert!(host.client.create(support::managed("digest", content, true)).await.is_err());
    assert!(host.client.get(wire::PluginsGetRequest { name: "digest".into(), at: None }).await.is_err());
    host.close().await;
}
