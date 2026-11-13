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
use harmos_protocol::{v1 as wire, v1};
use harmos_runtime::{
    Runtime,
    clock::Clock,
    plugin::manager::{self, Manager, PluginManagerSettings, PublishedContent},
    storage::{StorageBackends, StorageManager},
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Client = wire::plugins_client::PluginsClient<tonic::transport::Channel>;

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
        let storage = StorageManager::with_catalog(
            manager::catalog(),
            StorageBackends {
                primary: backends.data.clone(),
                state: backends.recovery.clone(),
            },
            storage_settings(),
        )
        .unwrap();
        let manager = Manager::new(storage.clone(), Arc::new(PublishedContent(storage.clone())), settings(root)).unwrap();
        let (clock, _) = Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
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
    pub async fn upload(&self, name: &str, bytes: Vec<u8>) -> wire::PluginManaged {
        let digest = Sha256::digest(&bytes).to_vec();
        let mut client = wire::storage_client::StorageClient::new(self.channel.clone());
        let window = self.storage.mutation_window().unwrap().id.to_string();
        let mutation = |action: &str| {
            Some(harmos_protocol::storage::MutationId {
                window: window.clone(),
                key: format!("{name}-{action}"),
            })
        };
        // Create the Upload and Append Ordered Offsets Through the Client Face.
        let production = client
            .create_file_production(wire::StorageCreateFileProductionRequest {
                metadata: Some(harmos_protocol::storage::FileMetadata {
                    id: name.into(),
                    scope: None,
                    media_type: "application/x-tar".into(),
                    expected_length: Some(bytes.len() as u64),
                    expected_sha256: Some(digest.clone()),
                }),
                mutation: mutation("create"),
            })
            .await
            .unwrap()
            .into_inner()
            .production
            .unwrap();
        let session = production.session;
        let mut offset = 0;
        for chunk in bytes.chunks(self.storage.settings().response.batch_bytes.get() / 2) {
            client
                .append_file_bytes(wire::StorageAppendFileBytesRequest {
                    session: session.clone(),
                    offset,
                    data: chunk.to_vec(),
                    mutation: mutation(&format!("append-{offset}")),
                })
                .await
                .unwrap();
            offset += chunk.len() as u64;
        }
        // Get Confirms Resume Progress Before Publication.
        let progress = client
            .get_file_production(wire::StorageGetFileProductionRequest {
                file: Some(harmos_protocol::storage::StagedFile {
                    id: name.into(),
                    history: session.as_ref().unwrap().history.clone(),
                }),
                at: None,
            })
            .await
            .unwrap()
            .into_inner();
        assert_eq!(progress.accepted_length, bytes.len() as u64);
        let content = client
            .publish_file_production(wire::StoragePublishFileProductionRequest {
                session,
                length: bytes.len() as u64,
                sha256: digest.clone(),
                mutation: mutation("publish"),
            })
            .await
            .unwrap()
            .into_inner()
            .file
            .unwrap();

        wire::PluginManaged { content: Some(content), sha256: digest }
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
        page_tokens: nz(64),
        watch_capacity: nz(16),
        page_size: nz(16),
        response_bytes: nz(1024 * 1024),
    }
}
pub fn remote(name: &str, url: &str) -> wire::PluginsCreateRequest {
    wire::PluginsCreateRequest {
        request_id: name.into(),
        name: name.into(),
        placement: Some(wire::PluginPlacement {
            kind: Some(wire::plugin_placement::Kind::Remote(wire::PluginRemote { url: url.into() })),
        }),
        value: Some(wire::PluginValue { enabled: true }),
    }
}
pub fn managed(name: &str, content: wire::PluginManaged, enabled: bool) -> wire::PluginsCreateRequest {
    wire::PluginsCreateRequest {
        request_id: name.into(),
        name: name.into(),
        placement: Some(wire::PluginPlacement {
            kind: Some(wire::plugin_placement::Kind::Managed(content)),
        }),
        value: Some(wire::PluginValue { enabled }),
    }
}
pub fn page() -> wire::PluginsListRequest {
    wire::PluginsListRequest {
        at: None,
        page: Some(v1::PageRequest {
            page_size: 16,
            max_bytes: 1024 * 1024,
            page_token: String::new(),
        }),
    }
}
pub async fn state(
    watch: &mut tonic::Streaming<wire::PluginsWatchResponse>, name: &str, state: wire::PluginState, activation: u64,
) -> wire::PluginEntry {
    loop {
        let response = watch.message().await.unwrap().expect("plugin state event");
        let entry = match response.event {
            Some(wire::plugins_watch_response::Event::Snapshot(snapshot)) => snapshot.entries.into_iter().find(|entry| entry.name == name),
            Some(wire::plugins_watch_response::Event::Change(wire::PluginChange {
                kind: Some(wire::plugin_change::Kind::Entry(entry)),
            })) => Some(entry),
            Some(wire::plugins_watch_response::Event::ResyncRequired(error)) => panic!("{error}"),
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
