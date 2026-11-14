/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Manager Story Mechanics

/* === Modules === */

mod process;

/* === Re-Exports === */

pub use process::Process;

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
// Third Party Imports
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};
// Local Imports
use super::super::support as placement;
use crate::storage::fixture::{Data, Recovery, history, storage_settings};
use harmos_runtime::{
    Runtime,
    clock::Clock,
    plugin::manager::{self, Manager, PluginManagerSettings},
    storage::{StorageBackends, StorageManager},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Client = harmos_protocol::client::plugins_client::PluginsClient<tonic::transport::Channel>;

/* === Structs === */

pub struct Host {
    pub runtime: Arc<Runtime<placement::App>>,
    pub manager: Manager,
    pub storage: StorageManager,
    pub client: Client,
    pub policy: Arc<Policy>,
    pub channel: tonic::transport::Channel,
    _listener: harmos_runtime::grpc::Attached,
}

pub struct Policy(pub AtomicBool);

pub struct Backends {
    pub data: Arc<Data>,
    pub recovery: Arc<Recovery>,
}

pub struct Peer {
    pub url: String,
    pub description: harmos_protocol::plugin::InstanceGetResponse,
    child: tokio::process::Child,
    drain: tokio::task::JoinHandle<()>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Backends {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Data::with_catalog(manager::catalog(), history())),
            recovery: Arc::new(Recovery::new(history())),
        }
    }
}
impl Host {
    pub async fn start(root: &Path, backends: &Backends) -> Self {
        let (clock, _) = Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
        Self::with_clock(root, backends, clock).await
    }
    pub async fn with_clock(root: &Path, backends: &Backends, clock: Clock) -> Self {
        let storage = StorageManager::with_catalog(
            manager::catalog(),
            StorageBackends {
                primary: backends.data.clone(),
                state: backends.recovery.clone(),
            },
            storage_settings(),
        )
        .unwrap();
        let manager = Manager::new(storage.clone(), settings(root)).unwrap();
        let runtime = crate::support::booted(Default::default(), |setup| {
            setup.settings().clock(clock).storage(&storage).plugins(manager.clone());
        })
        .await;
        let runtime = Arc::new(runtime);
        crate::support::app::register(&runtime, harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default())
            .await
            .unwrap();
        manager.start(&runtime).await.unwrap();
        let policy = Arc::new(Policy(AtomicBool::new(true)));
        runtime.operations.register_authorizer(policy.clone()).unwrap();
        let transport = harmos_runtime::grpc::Settings {
            address: "127.0.0.1:0".parse().unwrap(),
            reflection: false,
            tls: None,
            connections: nz(8),
            calls_per_connection: nz(16),
            request_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            operations: harmos_runtime::grpc::server::Settings::default(),
        };
        let listener = runtime.grpc().attach(transport).await.unwrap();
        let channel =
            tonic::transport::Endpoint::from_shared(format!("http://{}", listener.local_addr())).unwrap().connect().await.unwrap();
        let client = Client::new(channel.clone());
        Self {
            runtime,
            manager,
            storage,
            client,
            policy,
            channel,
            _listener: listener,
        }
    }
    pub async fn close(self) {
        self.runtime.shutdown().await.unwrap();
    }
    pub async fn upload(&self, name: &str, bytes: Vec<u8>) -> harmos_protocol::client::PluginManaged {
        let digest = Sha256::digest(&bytes).to_vec();
        let mut production = self
            .storage
            .produce_file(harmos_storage::FileMetadata {
                id: harmos_runtime::Identifier::new(name).unwrap(),
                scope: None,
                media_type: "application/x-tar".into(),
                expected_length: Some(bytes.len() as u64),
                expected_sha256: Some(Sha256::digest(&bytes).into()),
            })
            .await
            .unwrap();
        production.write_all(bytes).await.unwrap();
        let file = production.finish().await.unwrap();
        self.storage.flush_until(&file.position).await.unwrap();
        harmos_protocol::client::PluginManaged {
            content: Some(file.into()),
            sha256: digest,
        }
    }
}

impl harmos_runtime::operations::authorization::Authorizer for Policy {
    fn authorize(
        &self, _: &harmos_runtime::operations::CallContext, provider: harmos_runtime::operations::authorization::Provider<'_>,
    ) -> harmos_runtime::Result<Option<harmos_runtime::operations::authorization::Authorization>> {
        Ok(self.0.load(Ordering::SeqCst).then(|| harmos_runtime::operations::authorization::Authorization::Full(provider.contract.clone())))
    }
}

impl Peer {
    pub async fn start() -> Self {
        let mut child = tokio::process::Command::new(placement::binary("harmos-conformance-plugin"))
            .arg("--remote")
            .env("HARMOS_ADDRESS", "127.0.0.1:0")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let url = loop {
            let line = lines.next_line().await.unwrap().expect("reference plugin readiness");
            if let Some(address) = line.strip_prefix("harmos listening ") {
                break format!("http://{address}");
            }
        };
        let drain = tokio::spawn(async move { while lines.next_line().await.ok().flatten().is_some() {} });
        let mut client = harmos_protocol::plugin::instance_client::InstanceClient::connect(url.clone()).await.unwrap();
        let description =
            client.get(harmos_protocol::plugin::InstanceGetRequest { revision: 1, ..Default::default() }).await.unwrap().into_inner();
        Self { url, description, child, drain }
    }
    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
    pub async fn close(mut self) {
        self.child.kill().await.unwrap();
        self.drain.abort();
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        self.drain.abort();
    }
}

/* === Public Functions === */

pub async fn installs_itself(program: &Path, arguments: &[&str]) {
    // Serve This Codebase's Plugin in Its Own Process
    let mut command = tokio::process::Command::from(crate::support::fixtures::command(program));
    let mut peer = command
        .args(arguments)
        .args(["--install-itself", "serve"])
        .env("HARMOS_ADDRESS", "127.0.0.1:0")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut output = BufReader::new(peer.stdout.take().unwrap()).lines();
    let line = output.next_line().await.unwrap().expect("the SDK announces its bound endpoint");
    let endpoint = format!("http://{}", line.strip_prefix("harmos listening ").expect("SDK readiness"));
    let drain = tokio::spawn(async move { while output.next_line().await.ok().flatten().is_some() {} });

    // Run Its Entry Program Against a Controlled Host
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let host = Host::start(root.path(), &backends).await;
    host.runtime.operations.register_catalog(harmos_operations::lifecycle_catalog(), Default::default()).await.unwrap();
    let address = format!("http://{}", host._listener.local_addr());
    let mut command = tokio::process::Command::from(crate::support::fixtures::command(program));
    let result = command.args(arguments).args(["--install-itself", &address, &endpoint]).kill_on_drop(true).output().await.unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "hello");
    assert!(host.client.clone().list(list()).await.unwrap().into_inner().entries.is_empty());

    // Settle the Host and the Owned External Process
    host.close().await;
    peer.kill().await.unwrap();
    peer.wait().await.unwrap();
    drain.await.unwrap();
}

pub fn language_binary(name: &str, local: &str) -> std::path::PathBuf {
    if std::env::var_os("STKR_TEST_PROFILE").is_some() {
        return crate::support::fixtures::binary(name, None);
    }
    crate::support::fixtures::workspace().join(local)
}

pub fn settings(root: &Path) -> PluginManagerSettings {
    PluginManagerSettings {
        namespace: "plugin-manager-story".into(),
        directory: root.join("plugins"),
        target: if cfg!(target_os = "windows") { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }.into(),
        placement: placement::settings(),
        bundle: Default::default(),
        entries: nz(16),
        create_receipts: nz(64),
        registry_bytes: nz(1024 * 1024),
        command_capacity: nz(16),
        retained_cuts: nz(64),
        watch_capacity: nz(16),
        response_bytes: nz(1024 * 1024),
    }
}
pub fn remote(name: &str, url: &str) -> harmos_protocol::client::PluginsCreateRequest {
    harmos_protocol::client::PluginsCreateRequest {
        request_id: name.into(),
        name: name.into(),
        placement: Some(harmos_protocol::client::PluginPlacement {
            kind: Some(harmos_protocol::client::plugin_placement::Kind::Remote(harmos_protocol::client::PluginRemote { url: url.into() })),
        }),
        value: Some(harmos_protocol::client::PluginValue { enabled: true }),
    }
}
pub fn managed(
    name: &str, content: harmos_protocol::client::PluginManaged, enabled: bool,
) -> harmos_protocol::client::PluginsCreateRequest {
    harmos_protocol::client::PluginsCreateRequest {
        request_id: name.into(),
        name: name.into(),
        placement: Some(harmos_protocol::client::PluginPlacement {
            kind: Some(harmos_protocol::client::plugin_placement::Kind::Managed(content)),
        }),
        value: Some(harmos_protocol::client::PluginValue { enabled }),
    }
}
pub fn list() -> harmos_protocol::client::PluginsListRequest {
    harmos_protocol::client::PluginsListRequest { at: None }
}
pub async fn state(
    watch: &mut tonic::Streaming<harmos_protocol::client::PluginsWatchResponse>, name: &str, state: harmos_protocol::client::PluginState,
    activation: u64,
) -> harmos_protocol::client::PluginEntry {
    loop {
        let response = watch.message().await.unwrap().expect("plugin state event");
        let entry = match response.event {
            Some(harmos_protocol::client::plugins_watch_response::Event::Snapshot(snapshot)) => {
                snapshot.entries.into_iter().find(|entry| entry.name == name)
            }
            Some(harmos_protocol::client::plugins_watch_response::Event::Change(harmos_protocol::client::PluginChange {
                kind: Some(harmos_protocol::client::plugin_change::Kind::Entry(entry)),
            })) => Some(entry),
            Some(harmos_protocol::client::plugins_watch_response::Event::ResyncRequired(error)) => panic!("{error}"),
            _ => None,
        };
        if let Some(entry) = entry
            && entry.name == name
            && entry.state == state as i32
            && entry.activation >= activation
        {
            return entry;
        }
    }
}
pub fn bundle(executable: bool) -> Vec<u8> {
    use harmos_packaging::bundle::{self, Executable, Facility, Identity, Source};
    let bytes = std::fs::read(placement::binary("harmos-conformance-plugin")).unwrap();
    let source = Source {
        path: if cfg!(target_os = "windows") { "bin/plugin.exe" } else { "bin/plugin" },
        bytes: &bytes,
        executable: executable.then(|| Executable {
            facility: Facility::Sidecar,
            target: settings(Path::new("/")).target,
            registrations: vec![],
        }),
    };
    bundle::build(
        Identity {
            name: "reference-plugin".into(),
            version: "1.0.0".parse().unwrap(),
            description: "Reference Plugin".into(),
            package: Default::default(),
        },
        &[source],
        Default::default(),
    )
    .unwrap()
    .bytes()
    .to_vec()
}
/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
