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
use std::fs;

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

// Local Imports
use super::support::{self, Backends, Host};
use harmos_conformance::host::plugins::settings_client::SettingsClient;
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
    eprintln!("Replacement: Start the Host");
    let mut host = Host::start(root.path(), &backends).await;
    eprintln!("Replacement: Upload the Original Bundle");
    let bundle = host.upload("original", support::bundle(true)).await;
    eprintln!("Replacement: Open the Managed Entry Watch");
    let mut watch = host.client.watch(host::PluginsWatchRequest { name: Some("managed".into()), at: None }).await.unwrap().into_inner();
    eprintln!("Replacement: Create the Managed Entry");
    host.client.create(support::managed("managed", bundle, true)).await.unwrap();
    eprintln!("Replacement: Wait for the Original Activation to Be Ready");
    let first = support::state(&mut watch, "managed", host::PluginState::Ready, 1).await;
    eprintln!("Replacement: Retain the Original Process Observer");
    let original = support::Process::observe(first.process.as_ref().unwrap().id).await;
    let mut settings = SettingsClient::new(host.channel.clone());
    eprintln!("Replacement: List the Original Settings");
    let initial = settings.list(plugins::SettingsListRequest { name: Some(first.name.clone()), at: None }).await.unwrap().into_inner();

    let mut value = initial.value.unwrap();
    value.values.get_mut("count").unwrap().value = Some(Value::Integer(9));
    eprintln!("Replacement: Set Custom Settings");
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

    // Invalid Staging Leaves Both the Entry Revision and Its Serving Process
    eprintln!("Replacement: Upload the Invalid Bundle");
    let invalid = host.upload("invalid", support::bundle(false)).await;
    let invalid = host::PluginsReplaceRequest {
        name: first.name.clone(),
        placement: Some(host::PluginPlacement {
            kind: Some(host::plugin_placement::Kind::Managed(invalid)),
        }),
        expected_revision: first.revision.clone(),
        request_id: "invalid".into(),
    };
    eprintln!("Replacement: Reject Invalid Staging");
    assert!(host.client.replace(invalid).await.is_err());
    eprintln!("Replacement: Get the Retained Entry");
    let retained =
        host.client.get(host::PluginsGetRequest { name: first.name.clone(), at: None }).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(retained.revision, first.revision);
    assert_eq!(retained.process, first.process);

    // Replace in Place and Replay Without Starting Another Activation
    eprintln!("Replacement: Upload the Replacement Bundle");
    let bundle = host.upload("replacement", support::bundle(true)).await;
    let request = host::PluginsReplaceRequest {
        name: first.name.clone(),
        placement: Some(host::PluginPlacement {
            kind: Some(host::plugin_placement::Kind::Managed(bundle)),
        }),
        expected_revision: first.revision,
        request_id: "replace".into(),
    };
    eprintln!("Replacement: Replace the Serving Process");
    let replaced = host.client.replace(request.clone()).await.unwrap().into_inner();
    let next = replaced.entry.as_ref().unwrap();
    assert_eq!(next.name, first.name);
    assert!(next.activation > first.activation);
    assert_ne!(next.process.as_ref().unwrap().id, first.process.as_ref().unwrap().id);
    assert!(replaced.receipt.as_ref().unwrap().completed);
    eprintln!("Replacement: Replay the Replacement Receipt");
    assert_eq!(host.client.replace(request.clone()).await.unwrap().into_inner(), replaced);
    eprintln!("Replacement: Wait for the Original Process to Exit");
    original.assert_exited().await;
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 1);
    eprintln!("Replacement: List Settings After Replacement");
    let after = settings.list(plugins::SettingsListRequest { name: Some(next.name.clone()), at: None }).await.unwrap().into_inner();
    assert_eq!(after.value, before.value);
    assert_eq!(after.revision, before.revision);
    let mut changed = request;
    changed.expected_revision = next.revision.clone();
    eprintln!("Replacement: Reject Changed Receipt Replay");
    assert!(host.client.replace(changed).await.is_err());
    eprintln!("Replacement: Stop the Host");
    host.close().await;
}

#[tokio::test]
async fn failed_candidate_readiness_reaps_before_removing_its_bundle() {
    // Keep an Admitted Bundle Serving While a Candidate Waits for Readiness
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    eprintln!("Candidate Readiness: Start the Host");
    let mut host = Host::start(root.path(), &backends).await;
    eprintln!("Candidate Readiness: Upload the Original Bundle");
    let bundle = host.upload("original", support::bundle(true)).await;
    eprintln!("Candidate Readiness: Open the Managed Entry Watch");
    let mut watch = host.client.watch(host::PluginsWatchRequest { name: Some("managed".into()), at: None }).await.unwrap().into_inner();
    eprintln!("Candidate Readiness: Create the Managed Entry");
    host.client.create(support::managed("managed", bundle, true)).await.unwrap();
    eprintln!("Candidate Readiness: Wait for the Original Activation to Be Ready");
    let first = support::state(&mut watch, "managed", host::PluginState::Ready, 1).await;
    eprintln!("Candidate Readiness: Retain the Original Activation");
    let original = host.manager.activation("managed").await.unwrap();
    eprintln!("Candidate Readiness: Bind the Startup Control Listener");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    eprintln!("Candidate Readiness: Upload the Controlled Candidate Bundle");
    let bundle = host.upload("candidate", support::bundle_with_startup(true, Some(&listener.local_addr().unwrap().to_string()))).await;
    let mut client = host.client.clone();
    let replacement = tokio::spawn(async move {
        eprintln!("Candidate Readiness: Start Replacement");
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
    eprintln!("Candidate Readiness: Accept the Candidate Startup Connection");
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    eprintln!("Candidate Readiness: Read the Candidate Process ID");
    control.read_line(&mut id).await.unwrap();
    eprintln!("Candidate Readiness: Retain the Candidate Process Observer");
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Reject the Candidate and Inspect Physical Exit Before Its Directory Disappears
    eprintln!("Candidate Readiness: Reject the Candidate");
    control.get_mut().write_all(b"fail\n").await.unwrap();
    eprintln!("Candidate Readiness: Wait for Replacement Failure");
    assert!(replacement.await.unwrap().is_err());
    eprintln!("Candidate Readiness: Wait for the Candidate Process to Exit");
    process.assert_exited().await;
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 1);
    eprintln!("Candidate Readiness: Query the Original Activation");
    assert!(original.get().await.is_ok());
    eprintln!("Candidate Readiness: Stop the Host");
    host.close().await;
}
