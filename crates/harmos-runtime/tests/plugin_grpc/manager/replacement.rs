/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Bundle Replacement Journey

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

// Local Imports
use super::support::{self, Backends, Host};
use harmos_protocol::host;
use harmos_protocol::host::plugins;
use harmos_protocol::model::setting_value::Value;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn replacement_keeps_the_entry_and_failed_staging_keeps_its_process() {
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let bundle = host.upload("original", support::bundle(true)).await;
    let mut watch = host.client.watch(host::PluginsWatchRequest { name: Some("managed".into()), at: None }).await.unwrap().into_inner();
    host.client.create(support::managed("managed", bundle, true)).await.unwrap();
    let first = support::state(&mut watch, "managed", host::PluginState::Ready, 1).await;
    let original = support::Process::observe(first.process.as_ref().unwrap().id).await;
    let mut settings = harmos_conformance::host::plugins::settings_client::SettingsClient::new(host.channel.clone());
    let initial = settings.list(plugins::SettingsListRequest { name: Some(first.name.clone()), at: None }).await.unwrap().into_inner();

    let mut value = initial.value.unwrap();
    value.values.get_mut("count").unwrap().value = Some(Value::Integer(9));
    let before = settings
        .set(plugins::SettingsSetRequest {
            name: first.name.clone(),
            value: Some(value),
            expected_revision: initial.revision,
            request_id: "custom-settings".into(),
        })
        .await
        .unwrap()
        .into_inner();

    // Invalid Staging Leaves Both The Entry Revision And Its Serving Process.
    let invalid = host.upload("invalid", support::bundle(false)).await;
    let invalid = host::PluginsReplaceRequest {
        name: first.name.clone(),
        placement: Some(host::PluginPlacement {
            kind: Some(host::plugin_placement::Kind::Managed(invalid)),
        }),
        expected_revision: first.revision.clone(),
        request_id: "invalid".into(),
    };
    assert!(host.client.replace(invalid).await.is_err());
    let retained =
        host.client.get(host::PluginsGetRequest { name: first.name.clone(), at: None }).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(retained.revision, first.revision);
    assert_eq!(retained.process, first.process);

    // Replace In Place And Replay Without Starting Another Activation.
    let bundle = host.upload("replacement", support::bundle(true)).await;
    let request = host::PluginsReplaceRequest {
        name: first.name.clone(),
        placement: Some(host::PluginPlacement {
            kind: Some(host::plugin_placement::Kind::Managed(bundle)),
        }),
        expected_revision: first.revision,
        request_id: "replace".into(),
    };
    let replaced = host.client.replace(request.clone()).await.unwrap().into_inner();
    let next = replaced.entry.as_ref().unwrap();
    assert_eq!(next.name, first.name);
    assert!(next.activation > first.activation);
    assert_ne!(next.process.as_ref().unwrap().id, first.process.as_ref().unwrap().id);
    assert!(replaced.receipt.as_ref().unwrap().completed);
    assert_eq!(host.client.replace(request.clone()).await.unwrap().into_inner(), replaced);
    original.assert_exited().await;
    assert_eq!(std::fs::read_dir(root.path().join("plugins")).unwrap().count(), 1);
    let after = settings.list(plugins::SettingsListRequest { name: Some(next.name.clone()), at: None }).await.unwrap().into_inner();
    assert_eq!(after.value, before.value);
    assert_eq!(after.revision, before.revision);
    let mut changed = request;
    changed.expected_revision = next.revision.clone();
    assert!(host.client.replace(changed).await.is_err());
    host.close().await;
}

#[tokio::test]
async fn failed_candidate_readiness_reaps_before_removing_its_bundle() {
    // Keep an Admitted Bundle Serving While a Candidate Waits for Readiness
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let bundle = host.upload("original", support::bundle(true)).await;
    let mut watch = host.client.watch(host::PluginsWatchRequest { name: Some("managed".into()), at: None }).await.unwrap().into_inner();
    host.client.create(support::managed("managed", bundle, true)).await.unwrap();
    let first = support::state(&mut watch, "managed", host::PluginState::Ready, 1).await;
    let original = host.manager.activation("managed").await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bundle = host.upload("candidate", support::bundle_with_startup(true, Some(&listener.local_addr().unwrap().to_string()))).await;
    let mut client = host.client.clone();
    let replacement = tokio::spawn(async move {
        client
            .replace(host::PluginsReplaceRequest {
                name: first.name,
                placement: Some(host::PluginPlacement {
                    kind: Some(host::plugin_placement::Kind::Managed(bundle)),
                }),
                expected_revision: first.revision,
                request_id: "candidate".into(),
            })
            .await
    });
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    control.read_line(&mut id).await.unwrap();
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Reject the Candidate and Inspect Physical Exit Before Its Directory Disappears
    control.get_mut().write_all(b"fail\n").await.unwrap();
    assert!(replacement.await.unwrap().is_err());
    process.assert_exited().await;
    assert_eq!(std::fs::read_dir(root.path().join("plugins")).unwrap().count(), 1);
    assert!(original.get().await.is_ok());
    host.close().await;
}
