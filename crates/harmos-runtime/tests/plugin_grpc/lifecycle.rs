/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Lifecycle Operations Through Python

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{fs, num::NonZeroUsize, path::Path, process::Stdio, time::Duration};

// Third Party Imports
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
};

// Local Imports
use super::{manager, support};
use harmos_conformance::host::invocations_client::InvocationsClient;
use harmos_packaging::bundle::{self, Identity};
use harmos_protocol::host::InvocationsCreateRequest;
use harmos_protocol::host::{
    PluginState, PluginValue, PluginsDeleteRequest, PluginsGetRequest, PluginsRestartRequest, PluginsSetRequest, PluginsWatchRequest,
};
use harmos_runtime::{
    Runtime,
    plugin::grpc::{Attached, Placement},
};
use tonic::transport::Endpoint;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Python {
    process: Child,
    output: Lines<BufReader<ChildStdout>>,
    address: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Python {
    async fn start(mode: &str) -> Self {
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/python");
        let executable = if cfg!(target_os = "windows") { ".venv/Scripts/python.exe" } else { ".venv/bin/python" };
        let mut process = Command::new(sdk.join(executable))
            .args(["-m", "tests.fixtures.lifecycle", mode])
            .current_dir(sdk)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut output = BufReader::new(process.stdout.take().unwrap()).lines();
        let line = output.next_line().await.unwrap().unwrap();
        let address = format!("http://{}", line.strip_prefix("listening:").unwrap());
        Self { process, output, address }
    }

    async fn command(&mut self, command: &str) {
        let input = self.process.stdin.as_mut().unwrap();
        input.write_all(format!("{command}\n").as_bytes()).await.unwrap();
        input.flush().await.unwrap();
    }

    async fn event(&mut self, expected: &str) {
        assert_eq!(self.output.next_line().await.unwrap().as_deref(), Some(expected));
    }

    async fn close(mut self) {
        self.command("quit").await;
        assert!(self.process.wait().await.unwrap().success());
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn initialize(runtime: &Runtime<support::App>, plugin: &mut Python) -> Attached {
    let attaching = runtime.attach_grpc(Placement::Remote(plugin.address.clone()), support::settings());
    tokio::pin!(attaching);
    tokio::select! {
        result = &mut attaching => panic!("Initialize Before Activation: {:?}", result.err()),
        () = plugin.event("initialize") => {},
    }
    assert!(matches!(runtime.operations.submit_encoded("lifecycle.state", Vec::new()), Err(harmos_runtime::Error::NotReady { .. })));
    plugin.command("release").await;
    let attached = attaching.await.unwrap();
    plugin.event("initialized").await;
    attached
}

/* === Tests === */

#[tokio::test]
async fn managed_python_removal() {
    // Install a Managed Python Plugin Through the Real Manager.
    let root = tempfile::tempdir().unwrap();
    let finalized = root.path().join("finalized");
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/python").canonicalize().unwrap();
    let launch = vec![
        "uv".into(),
        "run".into(),
        "--locked".into(),
        "--no-sync".into(),
        "--directory".into(),
        sdk.to_str().unwrap().into(),
        "python".into(),
        "-m".into(),
        "tests.fixtures.lifecycle".into(),
        "managed".into(),
        finalized.to_str().unwrap().into(),
    ];
    let bundle = bundle::build(
        Identity {
            name: "lifecycle".into(),
            version: "1.0.0".parse().unwrap(),
            description: "Lifecycle Plugin".into(),
            package: Default::default(),
        },
        &[],
        Some(&launch),
        Default::default(),
    )
    .unwrap();
    let backends = manager::support::Backends::new();
    let mut host = manager::support::Host::start(root.path(), &backends).await;
    let content = host.upload("lifecycle", bundle.bytes().to_vec()).await;
    let mut watch = host.client.watch(PluginsWatchRequest { name: Some("lifecycle".into()), at: None }).await.unwrap().into_inner();
    host.client.create(manager::support::managed("lifecycle", content, true)).await.unwrap();
    let ready = manager::support::state(&mut watch, "lifecycle", PluginState::Ready, 1).await;
    let process = manager::support::Process::observe(ready.process.unwrap().id).await;

    // Removal Finalizes Once, Delivers Stop and Reaps Within the Placement Grace.
    host.client
        .delete(PluginsDeleteRequest {
            name: "lifecycle".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    process.assert_exited().await;
    assert_eq!(fs::read_to_string(finalized).unwrap(), "1");
    assert!(host.client.list(manager::support::list()).await.unwrap().into_inner().entries.is_empty());
    assert_eq!(fs::read_dir(root.path().join("plugins")).unwrap().count(), 0);
    drop(watch);
    host.close().await;
}

#[tokio::test]
async fn initialize_precedes_ready_and_state_and_hooks_are_hidden_from_clients() {
    // Admit Only After Initialize Completes.
    let runtime = support::runtime(None).await;
    let mut plugin = Python::start("success").await;
    let attached = initialize(&runtime, &mut plugin).await;
    let activation = attached.activation().unwrap();
    assert!(activation.is_connected());

    // Invoke With the Instance State Acquired During Initialize.
    let response = runtime.operations.call_encoded("lifecycle.state", Vec::new()).await.unwrap();
    assert_eq!(response, super::invoke_support::payload(b"initialized"));
    plugin.event("operation").await;
    plugin.event("settled").await;

    // Lifecycle Entries Never Enter Ordinary Host or Client Routing.
    runtime.operations.register_catalog(harmos_conformance::product::catalog(), Default::default()).await.unwrap();
    let listener = runtime
        .grpc()
        .attach(harmos_runtime::grpc::Settings {
            operations: Default::default(),
            reflection: false,
            address: "127.0.0.1:0".parse().unwrap(),
            tls: None,
            connections: NonZeroUsize::new(4).unwrap(),
            calls_per_connection: NonZeroUsize::new(8).unwrap(),
            request_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(2),
        })
        .await
        .unwrap();
    let channel = Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
    let mut client = InvocationsClient::new(channel);
    for name in ["lifecycle.initialize", "lifecycle.finalize"] {
        let contract =
            activation.description.declarations.as_ref().unwrap().operations.iter().find(|operation| operation.id == name).unwrap();
        assert!(runtime.operations.submit_encoded(name, Vec::new()).is_err());
        let request = InvocationsCreateRequest {
            operation: name.into(),
            fingerprint: contract.fingerprint.clone(),
            ..Default::default()
        };
        assert!(client.create(request).await.is_err());
    }

    // Finalize Exactly Once Across Repeated Stops.
    attached.stop("Complete").await.unwrap();
    plugin.event("finalize:0:1").await;
    attached.stop("Again").await.unwrap();
    plugin.command("state").await;
    plugin.event("state:0:1").await;
    runtime.stop().await.unwrap();
    plugin.close().await;
}

#[tokio::test]
async fn failed_initialize_reports_its_failure_and_finalizes_once() {
    let runtime = support::runtime(None).await;
    let mut plugin = Python::start("failure").await;
    let attaching = runtime.attach_grpc(Placement::Remote(plugin.address.clone()), support::settings());
    tokio::pin!(attaching);
    tokio::select! {
        result = &mut attaching => panic!("Wait for Initialize: {:?}", result.err()),
        () = plugin.event("initialize") => {},
    }
    plugin.command("release").await;
    let error = attaching.await.err().unwrap();
    let harmos_runtime::plugin::grpc::Error::Lifecycle { source, .. } = error else {
        panic!("Retain the Hook Failure");
    };
    let failure = harmos_protocol::model::Failure::from(source.failure().unwrap());
    assert_eq!(failure.code, "acquire_failed");
    assert_eq!(failure.classification(), harmos_protocol::model::Classification::Permanent);
    plugin.event("initialized").await;
    plugin.event("finalize:0:1").await;
    plugin.command("state").await;
    plugin.event("state:0:1").await;
    assert!(runtime.operations.submit_encoded("lifecycle.state", Vec::new()).is_err());
    runtime.stop().await.unwrap();
    plugin.close().await;
}

#[tokio::test]
async fn cancelled_initialize_settles_before_finalize_once() {
    let runtime = support::runtime(None).await;
    let mut plugin = Python::start("success").await;
    let mut attaching = Box::pin(runtime.attach_grpc(Placement::Remote(plugin.address.clone()), support::settings()));
    tokio::select! {
        result = &mut attaching => panic!("Wait for Initialize: {:?}", result.err()),
        () = plugin.event("initialize") => {},
    }
    drop(attaching);
    plugin.event("initialized").await;
    plugin.event("finalize:0:1").await;
    runtime.stop().await.unwrap();
    plugin.command("state").await;
    plugin.event("state:0:1").await;
    plugin.close().await;
}

#[tokio::test]
async fn stop_settles_inflight_operations_before_finalize() {
    let runtime = support::runtime(None).await;
    let mut plugin = Python::start("success").await;
    let attached = initialize(&runtime, &mut plugin).await;
    let call = runtime.operations.submit_encoded("lifecycle.state", super::invoke_support::payload(b"hold")).unwrap();
    plugin.event("operation").await;
    attached.stop("Cancel Work").await.unwrap();
    assert!(call.await.is_err());
    plugin.event("settled").await;
    plugin.event("finalize:0:1").await;
    runtime.stop().await.unwrap();
    plugin.close().await;
}

#[tokio::test]
async fn shutdown_finalizes_and_finalize_failure_does_not_prevent_stop() {
    let runtime = support::runtime(None).await;
    let mut plugin = Python::start("finalize_failure").await;
    let attached = initialize(&runtime, &mut plugin).await;
    let error = attached.stop("Release").await.unwrap_err();
    let harmos_runtime::plugin::grpc::Error::Lifecycle { source, .. } = error.as_ref() else {
        panic!("Retain the Finalize Failure");
    };
    let failure = harmos_protocol::model::Failure::from(source.failure().unwrap());
    assert_eq!(failure.code, "release_failed");
    assert!(attached.status().borrow().failure.is_some());
    plugin.event("finalize:0:1").await;
    runtime.stop().await.unwrap();
    plugin.close().await;

    let runtime = support::runtime(None).await;
    let mut plugin = Python::start("success").await;
    let _attached = initialize(&runtime, &mut plugin).await;
    runtime.stop().await.unwrap();
    plugin.event("finalize:0:1").await;
    plugin.close().await;
}

#[tokio::test]
async fn manager_publishes_ready_only_after_initialize() {
    // Register the Remote Entry While Its Initialize Is Held.
    let root = tempfile::tempdir().unwrap();
    let backends = manager::support::Backends::new();
    let mut host = manager::support::Host::start(root.path(), &backends).await;
    let mut plugin = Python::start("settings").await;
    let mut watch = host.client.watch(PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(manager::support::remote("lifecycle", &plugin.address)).await.unwrap();
    plugin.event("initialize").await;
    let entry = host.client.get(PluginsGetRequest { name: "lifecycle".into(), at: None }).await.unwrap().into_inner().entry.unwrap();
    assert_eq!(entry.state, PluginState::Connecting as i32);
    assert!(host.manager.activation("lifecycle").await.is_err());

    // Publish Ready Only After the Hook Returns.
    plugin.command("release").await;
    plugin.event("initialized").await;
    let ready = manager::support::state(&mut watch, "lifecycle", PluginState::Ready, 1).await;
    assert!(ready.failure.is_none());
    assert!(host.manager.activation("lifecycle").await.is_ok());
    host.client
        .restart(PluginsRestartRequest {
            name: ready.name.clone(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    plugin.event("finalize:0:1").await;
    plugin.event("initialize").await;
    plugin.event("initialized").await;
    let ready = manager::support::state(&mut watch, "lifecycle", PluginState::Ready, 2).await;
    assert!(ready.failure.is_none());

    let stopped = host
        .client
        .set(PluginsSetRequest {
            name: ready.name,
            expected_revision: ready.revision,
            value: Some(PluginValue { enabled: false }),
        })
        .await
        .unwrap()
        .into_inner()
        .entry
        .unwrap();
    assert_eq!(stopped.state, PluginState::Stopped as i32);
    plugin.event("finalize:0:2").await;
    host.client
        .set(PluginsSetRequest {
            name: stopped.name,
            expected_revision: stopped.revision,
            value: Some(PluginValue { enabled: true }),
        })
        .await
        .unwrap();
    plugin.event("initialize").await;
    plugin.event("initialized").await;
    let ready = manager::support::state(&mut watch, "lifecycle", PluginState::Ready, 4).await;
    assert!(ready.failure.is_none());
    host.client
        .delete(PluginsDeleteRequest {
            name: ready.name,
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    plugin.event("finalize:0:3").await;
    let mut request = manager::support::remote("lifecycle", &plugin.address);
    request.request_id = "lifecycle-restored".into();
    host.client.create(request).await.unwrap();
    plugin.event("initialize").await;
    plugin.event("initialized").await;
    let ready = manager::support::state(&mut watch, "lifecycle", PluginState::Ready, 1).await;
    assert!(ready.failure.is_none());
    drop(watch);
    host.close().await;
    plugin.event("finalize:0:4").await;
    plugin.close().await;
}

#[tokio::test]
async fn manager_retains_the_initialize_failure() {
    let root = tempfile::tempdir().unwrap();
    let backends = manager::support::Backends::new();
    let mut host = manager::support::Host::start(root.path(), &backends).await;
    let mut plugin = Python::start("failure").await;
    let mut watch = host.client.watch(PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(manager::support::remote("lifecycle", &plugin.address)).await.unwrap();
    plugin.event("initialize").await;
    plugin.command("release").await;
    plugin.event("initialized").await;
    plugin.event("finalize:0:1").await;
    let entry = manager::support::state(&mut watch, "lifecycle", PluginState::Unavailable, 1).await;
    let failure = entry.failure.unwrap();
    assert_eq!(failure.code, "acquire_failed");
    assert_eq!(failure.classification, harmos_protocol::model::Classification::Permanent as i32);
    assert_eq!(failure.operation, "lifecycle.initialize");
    assert!(host.manager.activation("lifecycle").await.is_err());
    drop(watch);
    host.close().await;
    plugin.close().await;
}
