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

use super::support::{self, Backends, Host};
use harmos_protocol::client::{self as wire, plugins};

// Standard Library Imports

// Third Party Imports

// Local Imports

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
    let mut watch = host.client.watch(wire::PluginsWatchRequest { name: "managed".into(), at: None }).await.unwrap().into_inner();
    host.client.create(support::managed("managed", bundle, true)).await.unwrap();
    let first = support::state(&mut watch, "managed", wire::PluginState::Ready, 1).await;
    let original = support::Process::observe(first.process.as_ref().unwrap().id).await;
    let mut settings = plugins::settings_client::SettingsClient::new(host.channel.clone());
    let initial = settings.list(plugins::SettingsListRequest { name: first.name.clone(), at: None }).await.unwrap().into_inner();

    let mut value = initial.value.unwrap();
    value.values.get_mut("count").unwrap().value = Some(harmos_protocol::v1::setting_value::Value::Integer(9));
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
    let invalid = wire::PluginsReplaceRequest {
        name: first.name.clone(),
        placement: Some(wire::PluginPlacement {
            kind: Some(wire::plugin_placement::Kind::Managed(invalid)),
        }),
        expected_revision: first.revision.clone(),
        request_id: "invalid".into(),
    };
    assert!(host.client.replace(invalid).await.is_err());
    let retained =
        host.client.get(wire::PluginsGetRequest { name: first.name.clone(), at: None }).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(retained.revision, first.revision);
    assert_eq!(retained.process, first.process);

    // Replace In Place And Replay Without Starting Another Activation.
    let bundle = host.upload("replacement", support::bundle(true)).await;
    let request = wire::PluginsReplaceRequest {
        name: first.name.clone(),
        placement: Some(wire::PluginPlacement {
            kind: Some(wire::plugin_placement::Kind::Managed(bundle)),
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
    assert!(original.exited().await.success());
    assert_eq!(std::fs::read_dir(root.path().join("plugins")).unwrap().count(), 1);
    let after = settings.list(plugins::SettingsListRequest { name: next.name.clone(), at: None }).await.unwrap().into_inner();
    assert_eq!(after.value, before.value);
    assert_eq!(after.revision, before.revision);
    let mut changed = request;
    changed.expected_revision = next.revision.clone();
    assert!(host.client.replace(changed).await.is_err());
    host.close().await;
}
