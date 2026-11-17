/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose the Same Schematic Commands Across Real Carrier Boundaries.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io::Error;
use std::num::NonZeroU64;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{env, fs};

// Third Party Imports
use prost::Message;
use serde_json::Value;
use tempfile::TempDir;

// Local Imports
use harmos_runtime::guest::Host;
use harmos_runtime::plugin::{grpc, manager};
use harmos_runtime::{Application, Identifier, Lifecycle, Runtime, Setup, Startup, config, operations, state, storage};
use harmos_storage::memory::{Backend, RecoveryBackend};
use harmos_storage::{StorageFilesystemSettings, StorageRequestSettings, StorageResponseSettings, StorageSettings, StoreHistory};
use schematic_operations::protocol::plugin::{
    SchematicProposeRouteRequest, SchematicProposeRouteResponse, SchematicValidateRequest, SchematicValidateResponse,
};
use schematic_operations::protocol::{Point, Snapshot};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Arc<Runtime<Model>>,
    pub startup_elapsed: Duration,
    _directory: TempDir,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lifecycle for Model {
    type Config = ();
    type Error = Error;
    type Context = Startup<Self>;

    async fn prepare(_: (), _setup: &mut Setup<Self>) -> Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl Application for Model {
    type State = ();
    type Transaction = state::Never;
    type Record = ();
}

impl Fixture {
    pub async fn guest() -> Self {
        let (runtime, host_elapsed, directory) = runtime().await;
        let started = Instant::now();
        let bytes = fs::read(artifact("schematic-guest")).unwrap();
        let settings = config::PluginGuestSettings {
            stream_window: None,
            component_bytes: nz(64 * 1024 * 1024),
            memory_bytes: nz(256 * 1024 * 1024),
            // Concurrent Criterion Streams May Keep the Guest Busy Between Iterations.
            fuel: NonZeroU64::MAX,
            memories: nz(1),
            table_elements: nz(65_536),
            tables: nz(8),
            instances: nz(8),
        };
        let host = Host::with_settings(&runtime, settings).unwrap();
        let loaded = host.load(&bytes).unwrap();
        runtime.plugin.register_guest(loaded, plugin_settings(), operations::Lane::default()).await.unwrap();
        runtime.plugin.initialize().await.unwrap();
        Self {
            runtime,
            startup_elapsed: host_elapsed + started.elapsed(),
            _directory: directory,
        }
    }

    pub async fn command(&self) -> SchematicProposeRouteResponse {
        let request = SchematicProposeRouteRequest {
            from: Some(Point { x: 0, y: 0 }),
            to: Some(Point { x: 3, y: 4 }),
        };
        let payload =
            self.runtime.operations.call_encoded("schematic.plugin.v1.Schematic/ProposeRoute", request.encode_to_vec()).await.unwrap();
        SchematicProposeRouteResponse::decode(payload.as_slice()).unwrap()
    }
    pub async fn payload(&self, request: SchematicValidateRequest) -> SchematicValidateResponse {
        let payload =
            self.runtime.operations.call_encoded("schematic.plugin.v1.Schematic/Validate", request.encode_to_vec()).await.unwrap();
        SchematicValidateResponse::decode(payload.as_slice()).unwrap()
    }

    pub async fn finish(self) {
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

pub fn verify_route(answer: &SchematicProposeRouteResponse) {
    assert_eq!(answer.points, [Point { x: 0, y: 0 }, Point { x: 3, y: 0 }, Point { x: 3, y: 4 },]);
}

pub fn payload(symbols: usize) -> SchematicValidateRequest {
    let symbols = (0..symbols).map(|value| value as u64).collect();
    SchematicValidateRequest {
        schematic: Some(Snapshot { symbols, connections: Vec::new() }),
    }
}

pub fn artifact(name: &str) -> PathBuf {
    let executable = env::current_exe().unwrap();
    let fixtures = executable.parent().unwrap().parent().unwrap().join("fixtures");
    let stamp_path = fixtures.join("stamp.json");
    let bytes = fs::read(stamp_path).unwrap();
    let stamp: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(stamp["profile"], "bench");
    let target = format!("{}-{}", env::consts::OS, env::consts::ARCH);
    assert_eq!(stamp["target"], target);
    let relative = stamp["metadata"]["artifacts"][name].as_str().unwrap_or_else(|| panic!("missing bench artifact {name}"));
    fixtures.join(relative)
}

/* === Private Functions === */

async fn runtime() -> (Arc<Runtime<Model>>, Duration, TempDir) {
    let started = Instant::now();
    let preparation_bytes = nz(64 * 1024 * 1024);
    let directory = tempfile::tempdir().unwrap();
    let catalog = manager::catalog();
    let history = StoreHistory {
        store: Identifier::new("test-store").unwrap(),
        epoch: Identifier::new("test-epoch").unwrap(),
    };
    let storage = storage::StorageManager::with_catalog(
        catalog.clone(),
        storage::StorageBackends {
            primary: Arc::new(Backend::with_catalog(catalog, history.clone())),
            state: Arc::new(RecoveryBackend::new(history)),
        },
        StorageSettings {
            transfer: Default::default(),
            policy: Default::default(),
            filesystem: StorageFilesystemSettings {
                poll_interval: Duration::from_millis(100),
                max_entries: nz(100_000),
                event_capacity: nz(1024),
            },
            request: StorageRequestSettings {
                max_file_productions: nz(256),
                max_mutation_receipts: nz(1024),
                max_pending_queries: nz(64),
                max_active_queries: nz(4),
                max_pending_write_batches: nz(256),
                max_active_file_operations: nz(1),
                max_pending_write_bytes: nz(16 * 1024 * 1024),
            },
            response: StorageResponseSettings {
                batch_rows: nz(64),
                batch_bytes: nz(64 * 1024),
            },
        },
    )
    .unwrap();
    let entries = manager::Manager::new(
        storage.clone(),
        manager::PluginManagerSettings {
            namespace: "guest-benchmark".into(),
            directory: directory.path().join("plugins"),
            target: if cfg!(target_os = "windows") { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }.into(),
            placement: grpc::PlacementSettings {
                reconnect_delay: Duration::from_millis(100),
                stop_grace: Duration::from_secs(10),
                stream_window: nz(4),
                interval_window: Duration::from_secs(1),
                interval_lag: Default::default(),
            },
            bundle: Default::default(),
            entries: nz(16),
            create_receipts: nz(64),
            registry_bytes: nz(1024 * 1024),
            command_capacity: nz(16),
            retained_cuts: nz(64),
            watch_capacity: nz(16),
            response_bytes: nz(1024 * 1024),
        },
    )
    .unwrap();
    let runtime = Runtime::<Model>::start_with((), (), |setup| {
        setup.settings().storage(&storage).plugins(entries.clone());
        setup.catalog(harmos_operations::storage_catalog(), operations::Lane::default());
        setup.catalog(schematic_operations::transfer::files_catalog(), operations::Lane::default());
        setup.settings().operation_settings(operations::Settings {
            queued: nz(64),
            lifecycle_queued: nz(16),
            preparation_bytes: Some(preparation_bytes),
        });
    })
    .await
    .unwrap();
    let runtime = Arc::new(runtime);
    entries.start(&runtime).await.unwrap();
    let host_elapsed = started.elapsed();
    (runtime, host_elapsed, directory)
}

fn plugin_settings() -> config::PluginSettings {
    config::PluginSettings {
        configuration: b"null".to_vec(),
        requires: Vec::new(),
        grants: Vec::new(),
        host_operations: ["schematic.host.v1.Files/Hash", "harmos.plugin.v1.plugins.Settings/List"]
            .into_iter()
            .map(|id| Identifier::new(id).unwrap())
            .collect(),
        initialization_timeout: Duration::from_secs(30),
        shutdown_timeout: Duration::from_secs(30),
        encoding_memory_bytes: nz(4 * 1024 * 1024),
        restart: config::PluginRestartSettings {
            retries: 0,
            backoff_base: Duration::from_millis(100),
            backoff_cap: Duration::from_secs(5),
        },
    }
}

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
