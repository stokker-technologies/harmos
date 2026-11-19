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

// Standard Library Imports
use std::{env, fs};

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

// Local Imports
use super::support::{self, Backends, Host};
use crate::plugin_grpc;
use harmos_packaging::bundle::{self, Executable, Facility, Identity, Source};
use harmos_protocol::host::PluginState;
use harmos_protocol::host::PluginsDeleteRequest;
use harmos_protocol::host::PluginsRestartRequest;
use harmos_protocol::host::PluginsWatchRequest;
use harmos_protocol::host::{PluginPlacement, PluginsReplaceRequest, plugin_placement};
use harmos_protocol::host::{PluginValue, PluginsSetRequest};
use harmos_protocol::model::Failure;

/* === Tests === */

#[tokio::test]
async fn managed_bundle_version_survives_activation_replacement_and_manager_restart() {
    // Read the Executable Once for Both Launch Modes
    let bytes = fs::read(plugin_grpc::support::binary("harmos-conformance-plugin")).unwrap();
    for command in [false, true] {
        // Build Two Bundles Shared by the Enabled and Disabled Entries
        let root = tempfile::tempdir().unwrap();
        let path = format!("bin/plugin{}", env::consts::EXE_SUFFIX);
        let launch = command.then(|| vec![format!("./{path}")]);
        let sources = [Source {
            path: &path,
            bytes: &bytes,
            executable: Some(Executable {
                facility: Facility::Sidecar,
                target: support::settings(root.path()).target,
                registrations: vec![],
            }),
        }];
        let identity = Identity {
            name: "frozen-plugin".into(),
            version: "8.7.6".parse().unwrap(),
            description: "Frozen Plugin".into(),
            package: Default::default(),
        };
        let original = bundle::build(identity.clone(), &sources, launch.as_deref(), Default::default()).unwrap();
        let replacement_identity = Identity {
            version: "9.8.7".parse().unwrap(),
            ..identity.clone()
        };
        let replacement = bundle::build(replacement_identity.clone(), &sources, launch.as_deref(), Default::default()).unwrap();

        // Upload Each Immutable Bundle Once per Launch Mode
        let backends = Backends::new();
        let mut host = Host::start(root.path(), &backends).await;
        let original = host.upload("original", original.bytes().to_vec()).await;
        let replacement = host.upload("replacement", replacement.bytes().to_vec()).await;
        let mut entries = Vec::new();
        let mut processes = Vec::new();
        for enabled in [true, false] {
            // Activation Publishes the Bundle Version While Retaining Live Metadata
            let name = if enabled { "frozen-enabled" } else { "frozen-disabled" };
            let mut watch = host.client.watch(PluginsWatchRequest { name: Some(name.into()), at: None }).await.unwrap().into_inner();
            host.client.create(support::managed(name, original.clone(), true)).await.unwrap();
            let ready = support::state(&mut watch, name, PluginState::Ready, 1).await;
            let first = support::Process::observe(ready.process.as_ref().unwrap().id).await;
            let live = host.manager.activation(name).await.unwrap();
            assert_ne!(live.description.version, identity.version.to_string());
            assert_ne!(live.description.version, replacement_identity.version.to_string());
            assert_eq!(ready.description.as_ref().unwrap().version, identity.version.to_string());
            assert_eq!(ready.description.as_ref().unwrap().id, live.description.id);

            // Disable Reaps the Child and Retains the Original Bundle Description
            let (ready, first) = if enabled {
                (ready, Some(first))
            } else {
                let stopped = host
                    .client
                    .set(PluginsSetRequest {
                        name: ready.name,
                        value: Some(PluginValue { enabled: false }),
                        expected_revision: ready.revision,
                    })
                    .await
                    .unwrap()
                    .into_inner()
                    .entry
                    .unwrap();
                first.assert_exited().await;
                assert_eq!(stopped.state, PluginState::Stopped as i32);
                assert!(stopped.process.is_none());
                assert_eq!(stopped.description.as_ref().unwrap().version, identity.version.to_string());
                (stopped, None)
            };

            // Replacement Publishes Its Bundle Version and Reaps the Previous Child
            let replaced = host
                .client
                .replace(PluginsReplaceRequest {
                    name: ready.name,
                    placement: Some(PluginPlacement {
                        kind: Some(plugin_placement::Kind::Managed(replacement.clone())),
                    }),
                    expected_revision: ready.revision,
                    request_id: format!("replace-{name}"),
                })
                .await
                .unwrap()
                .into_inner()
                .entry
                .unwrap();
            if let Some(first) = first {
                first.assert_exited().await;
            }
            assert_eq!(replaced.description.as_ref().unwrap().version, replacement_identity.version.to_string());

            // Enable the Replaced Entry When Necessary and Observe Its Live Version
            let activating = if enabled {
                replaced
            } else {
                assert_eq!(replaced.state, PluginState::Stopped as i32);
                assert!(replaced.process.is_none());
                host.client
                    .set(PluginsSetRequest {
                        name: replaced.name,
                        value: Some(PluginValue { enabled: true }),
                        expected_revision: replaced.revision,
                    })
                    .await
                    .unwrap()
                    .into_inner()
                    .entry
                    .unwrap()
            };
            let ready = support::state(&mut watch, name, PluginState::Ready, activating.activation).await;
            assert_eq!(ready.description.as_ref().unwrap().version, replacement_identity.version.to_string());
            let second = support::Process::observe(ready.process.as_ref().unwrap().id).await;
            entries.push(ready);
            processes.push(second);
        }

        // Shutdown Reaps Every Replacement Before a New Manager Starts
        host.close().await;
        for second in processes {
            second.assert_exited().await;
        }

        // A New Manager Reconstructs Both Entries From Their Installed Manifests
        let mut host = Host::start(root.path(), &backends).await;
        let mut processes = Vec::new();
        for ready in entries {
            let mut watch = host.client.watch(PluginsWatchRequest { name: Some(ready.name.clone()), at: None }).await.unwrap().into_inner();
            let restored = support::state(&mut watch, &ready.name, PluginState::Ready, ready.activation + 1).await;
            assert_eq!(restored.description.as_ref().unwrap().version, replacement_identity.version.to_string());
            let live = host.manager.activation(&ready.name).await.unwrap();
            assert_ne!(live.description.version, replacement_identity.version.to_string());
            let third = support::Process::observe(restored.process.as_ref().unwrap().id).await;
            processes.push(third);
        }

        // The Restored Children Are Reaped Before the Next Launch Mode
        host.close().await;
        for third in processes {
            third.assert_exited().await;
        }
    }
}

#[tokio::test]
async fn client_manages_bundled_plugins() {
    // Upload a Bundle Through the Library's Published File Session
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let content = host.upload("bundle", support::bundle(true)).await;
    let mut watch = host.client.watch(PluginsWatchRequest { name: Some("managed".into()), at: None }).await.unwrap().into_inner();

    // Create a Managed Entry and Observe Its Process Identity
    host.client.create(support::managed("managed", content, true)).await.unwrap();
    let ready = support::state(&mut watch, "managed", PluginState::Ready, 1).await;
    let process = ready.process.as_ref().unwrap();
    let first_id = process.id;
    let first = support::Process::observe(first_id).await;
    assert!(!process.executable_digest.is_empty());
    assert!(process.started_at.is_some());

    // Kill Only That Process and Observe the Retained Failure
    assert!(first.kill().await.success());
    let unavailable = support::state(&mut watch, "managed", PluginState::Unavailable, ready.activation).await;
    assert_eq!(unavailable.name, ready.name);
    assert_eq!(unavailable.placement, ready.placement);
    assert!(unavailable.process.is_none());
    let failure = unavailable.failure.unwrap();
    assert!(!failure.message.is_empty());
    assert!(!failure.action.is_empty());

    // Restart Replaces the Activation and Starts a New Process
    let restarting = host
        .client
        .restart(PluginsRestartRequest {
            name: "managed".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    let ready = support::state(&mut watch, "managed", PluginState::Ready, restarting.activation).await;
    assert!(ready.activation > unavailable.activation);
    let second_id = ready.process.unwrap().id;
    let second = support::Process::observe(second_id).await;

    // Delete Awaits Process Exit and Removes Its Installed Files
    host.client
        .delete(PluginsDeleteRequest {
            name: "managed".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    assert!(second.exited().await.success());
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    assert!(host.client.list(support::list()).await.unwrap().into_inner().entries.is_empty());
    drop(watch);
    host.close().await;
}

#[tokio::test]
async fn cancelled_startup_is_reaped_before_immediate_delete() {
    // Hold the Managed Child Before It Announces Readiness
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let content = host.upload("starting", support::bundle_with_startup(true, Some(&listener.local_addr().unwrap().to_string()))).await;
    let entry = host.client.create(support::managed("starting", content, true)).await.unwrap().into_inner().entry.unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    control.read_line(&mut id).await.unwrap();
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Cancel Startup and Delete Without Advancing Time or Retrying
    let stopped = host
        .client
        .set(PluginsSetRequest {
            name: entry.name.clone(),
            value: Some(PluginValue { enabled: false }),
            expected_revision: entry.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    assert!(stopped.failure.is_none());
    process.assert_exited().await;
    host.client
        .delete(PluginsDeleteRequest {
            name: stopped.name,
            expected_revision: stopped.revision,
        })
        .await
        .unwrap();
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    host.close().await;
}

#[tokio::test]
async fn failed_readiness_is_reaped_before_immediate_delete() {
    // Retain the Child That Will Announce an Invalid Address
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let content = host.upload("failing", support::bundle_with_startup(true, Some(&listener.local_addr().unwrap().to_string()))).await;
    let mut watch = host.client.watch(PluginsWatchRequest { name: Some("failing".into()), at: None }).await.unwrap().into_inner();
    host.client.create(support::managed("failing", content, true)).await.unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    control.read_line(&mut id).await.unwrap();
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Fail Readiness While the Native Executable Remains Open
    control.get_mut().write_all(b"fail\n").await.unwrap();
    let failed = support::state(&mut watch, "failing", PluginState::Unavailable, 1).await;
    assert!(failed.failure.is_some());
    process.assert_exited().await;
    host.client
        .delete(PluginsDeleteRequest {
            name: failed.name,
            expected_revision: failed.revision,
        })
        .await
        .unwrap();
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    host.close().await;
}

#[tokio::test]
async fn shutdown_reaps_a_managed_child_during_startup() {
    // Observe a Native Child Held Before Readiness
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let content = host.upload("starting", support::bundle_with_startup(true, Some(&listener.local_addr().unwrap().to_string()))).await;
    host.client.create(support::managed("starting", content, true)).await.unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    control.read_line(&mut id).await.unwrap();
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Shutdown Completes Only After the Retained Native Process Has Exited
    host.close().await;
    process.assert_exited().await;
}

#[tokio::test]
async fn restart_during_startup_reaps_before_reusing_the_installation() {
    // Retain the First Child Before It Announces Readiness
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let content = host.upload("starting", support::bundle_with_startup(true, Some(&listener.local_addr().unwrap().to_string()))).await;
    let first = host.client.create(support::managed("starting", content, true)).await.unwrap().into_inner().entry.unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    control.read_line(&mut id).await.unwrap();
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Restart Reaps the First Child Before Starting the Same Executable Again
    let next = host
        .client
        .restart(PluginsRestartRequest {
            name: first.name,
            expected_revision: first.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    process.assert_exited().await;
    let (stream, _) = listener.accept().await.unwrap();
    let mut control = BufReader::new(stream);
    let mut id = String::new();
    control.read_line(&mut id).await.unwrap();
    let process = support::Process::observe(id.trim().parse().unwrap()).await;

    // Delete Directly During the Replacement Child's Startup
    host.client
        .delete(PluginsDeleteRequest {
            name: next.name,
            expected_revision: next.revision,
        })
        .await
        .unwrap();
    process.assert_exited().await;
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    host.close().await;
}

#[tokio::test]
async fn command_bundle_installs_starts_restarts_and_restores() {
    // Install an Interpreter and Script Without Native Executable Metadata.
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let program = format!("./bin/interpreter{}", env::consts::EXE_SUFFIX);
    let content = host.upload("command", support::launch_bundle(&program)).await;
    let mut watch = host.client.watch(PluginsWatchRequest { name: Some("command".into()), at: None }).await.unwrap().into_inner();
    host.client.create(support::managed("command", content, true)).await.unwrap();
    let ready = support::state(&mut watch, "command", PluginState::Ready, 1).await;
    let first = support::Process::observe(ready.process.as_ref().unwrap().id).await;

    // Restart Uses the Same Command and the Existing Process Contract.
    let restarted = host
        .client
        .restart(PluginsRestartRequest {
            name: ready.name.clone(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    first.assert_exited().await;
    let ready = support::state(&mut watch, "command", PluginState::Ready, restarted.activation).await;
    let second = support::Process::observe(ready.process.unwrap().id).await;
    drop(watch);
    host.close().await;
    second.assert_exited().await;

    // Persisted Command Entries Restore and Delete Through the Same Manager.
    let mut host = Host::start(root.path(), &backends).await;
    let mut watch = host.client.watch(PluginsWatchRequest { name: Some("command".into()), at: None }).await.unwrap().into_inner();
    let ready = support::state(&mut watch, "command", PluginState::Ready, 1).await;
    let third = support::Process::observe(ready.process.unwrap().id).await;
    host.client
        .delete(PluginsDeleteRequest {
            name: ready.name,
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    third.assert_exited().await;
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    drop(watch);
    host.close().await;
}

#[tokio::test]
async fn command_bundle_reports_missing_program_at_install_and_activation() {
    // A Missing Relative Program Refuses Even a Disabled Installation.
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let mut host = Host::start(root.path(), &backends).await;
    let content = host.upload("missing-program", support::launch_bundle("./bin/missing-interpreter")).await;
    let error = host.client.create(support::managed("missing-program", content, false)).await.unwrap_err();
    let detail = harmos_runtime::Failure::decode(error.details()).unwrap();
    let failure = Failure::from(&detail);
    assert!(failure.message.contains("missing-interpreter"), "{failure:?}");
    assert!(failure.action.contains("install ./bin/missing-interpreter"), "{failure:?}");
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);

    // Removing an Installed Program Produces the Same Actionable Activation Error.
    let program = format!("./bin/interpreter{}", env::consts::EXE_SUFFIX);
    let content = host.upload("removed-program", support::launch_bundle(&program)).await;
    let entry = host.client.create(support::managed("removed-program", content, false)).await.unwrap().into_inner().entry.unwrap();
    let directory = fs::read_dir(root.path().join("plugins")).unwrap().next().unwrap().unwrap().path();
    fs::remove_file(directory.join(program)).unwrap();
    let mut watch = host.client.watch(PluginsWatchRequest { name: Some(entry.name.clone()), at: None }).await.unwrap().into_inner();
    host.client
        .set(PluginsSetRequest {
            name: entry.name.clone(),
            expected_revision: entry.revision,
            value: Some(PluginValue { enabled: true }),
        })
        .await
        .unwrap();
    let failed = support::state(&mut watch, &entry.name, PluginState::Unavailable, 1).await;
    let failure = failed.failure.unwrap();
    assert!(failure.message.contains("interpreter"), "{failure:?}");
    assert!(failure.message.contains("install"), "{failure:?}");
    host.client
        .delete(PluginsDeleteRequest {
            name: failed.name,
            expected_revision: failed.revision,
        })
        .await
        .unwrap();
    drop(watch);
    host.close().await;
}
