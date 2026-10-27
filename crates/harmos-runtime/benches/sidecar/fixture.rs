/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compose The Same Schematic Commands Across Real Carrier Boundaries.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
#[cfg(feature = "plugin-guest")]
use std::num::NonZeroU64;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third Party Imports

// Local Imports
use harmos_runtime::Runtime;
use harmos_runtime::storage::*;
use schematic_domain::model::Snapshot;
use schematic_domain::operations::schematic::{propose, validate};
use sha2::{Digest, Sha256};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    pub startup_elapsed: Duration,
    storage: StorageManager,
    _storage_root: tempfile::TempDir,
}

/* === Enums === */

#[harmos_runtime::lanes]
pub enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

impl Fixture {
    #[cfg(feature = "plugin-sidecar")]
    #[allow(dead_code, reason = "Each Carrier Benchmark Selects Its Matching Constructor")]
    pub async fn sidecar() -> Self {
        let (runtime, storage, root, host_elapsed) = runtime().await;
        let started = Instant::now();
        let executable = harmos_runtime::sidecar::Executable::at(artifact("schematic-sidecar")).unwrap();
        let mut plugin = plugin_settings();
        plugin.host_operations.push(harmos_runtime::Identifier::new("host.publish").unwrap());
        let settings = harmos_runtime::config::PluginSidecarSettings { plugin, calls: nz(8) };
        runtime.plugin.register_sidecar::<schematic_domain::operations::transfer::Native>(executable, settings, Lanes::Open).await.unwrap();
        runtime.plugin.initialize().await.unwrap();
        Self { runtime, storage, _storage_root: root, startup_elapsed: host_elapsed + started.elapsed() }
    }

    #[cfg(feature = "plugin-guest")]
    #[allow(dead_code, reason = "Each Carrier Benchmark Selects Its Matching Constructor")]
    pub async fn guest() -> Self {
        let (runtime, storage, root, host_elapsed) = runtime().await;
        let started = Instant::now();
        let bytes = std::fs::read(artifact("schematic-guest")).unwrap();
        let settings = harmos_runtime::config::PluginGuestSettings {
            component_bytes: nz(64 * 1024 * 1024),
            memory_bytes: nz(256 * 1024 * 1024),
            fuel: NonZeroU64::new(2_000_000_000).unwrap(),
            memories: nz(1),
            table_elements: nz(65_536),
            tables: nz(8),
            instances: nz(8),
        };
        let host = harmos_runtime::guest::Host::with_settings(&runtime, settings).unwrap();
        let loaded = host.load(&bytes).unwrap();
        runtime
            .plugin
            .register_guest::<schematic_domain::operations::transfer::Guest>(loaded, plugin_settings(), Lanes::Open)
            .await
            .unwrap();
        runtime.plugin.initialize().await.unwrap();
        Self { runtime, storage, _storage_root: root, startup_elapsed: host_elapsed + started.elapsed() }
    }

    pub async fn command(&self) -> harmos_runtime::operations::Response<propose::Route> {
        self.runtime
            .operations
            .call(propose::ProposeRoute {
                from: schematic_domain::model::Point { x: 0, y: 0 },
                to: schematic_domain::model::Point { x: 3, y: 4 },
            })
            .await
            .unwrap()
    }

    pub async fn payload(&self, request: validate::ValidateSchematic) -> harmos_runtime::operations::Response<validate::Validation> {
        self.runtime.operations.call(request).await.unwrap()
    }

    pub async fn verify_filesystem(&self) {
        self.storage.start(&self.runtime).await.unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("carrier-hash.bin");
        let bytes = b"host filesystem through the carrier";
        std::fs::write(&path, bytes).unwrap();
        let request = schematic_domain::operations::transfer::Hash {
            request: harmos_runtime::storage::operations::HashFile {
                source: FileSource::Local(path),
                range: ByteRange { offset: 5, limit: Some(10) },
            },
        };
        let answer = self.runtime.operations.call(request).await.unwrap();
        assert_eq!(answer.bytes_read, 10);
        assert_eq!(answer.sha256, <[u8; 32]>::from(Sha256::digest(&bytes[5..15])));
        self.storage.close().await.unwrap();
        assert!(!self.storage.status().running);
    }

    pub async fn finish(self) {
        self.storage.close().await.unwrap();
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

pub fn verify_route(answer: &propose::Route) {
    assert_eq!(
        answer.points,
        [
            schematic_domain::model::Point { x: 0, y: 0 },
            schematic_domain::model::Point { x: 3, y: 0 },
            schematic_domain::model::Point { x: 3, y: 4 },
        ]
    );
}

pub fn payload(symbols: usize) -> validate::ValidateSchematic {
    let symbols = (0..symbols).map(|value| value as u64).collect();
    validate::ValidateSchematic { schematic: Snapshot { symbols, connections: Vec::new() } }
}

pub fn artifact(name: &str) -> PathBuf {
    let executable = std::env::current_exe().unwrap();
    let fixtures = executable.parent().unwrap().parent().unwrap().join("fixtures");
    let stamp_path = fixtures.join("stamp.json");
    let bytes = std::fs::read(stamp_path).unwrap();
    let stamp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(stamp["profile"], "bench");
    let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    assert_eq!(stamp["target"], target);
    let relative = stamp["metadata"]["artifacts"][name].as_str().unwrap_or_else(|| panic!("missing bench artifact {name}"));
    fixtures.join(relative)
}

/* === Private Functions === */

async fn runtime() -> (Runtime<Model>, StorageManager, tempfile::TempDir, Duration) {
    let started = Instant::now();
    let preparation_bytes = nz(64 * 1024 * 1024);
    let runtime = Runtime::<Model>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            queued: nz(64),
            lifecycle_queued: nz(16),
            preparation_bytes: Some(preparation_bytes),
        })
        .runtime()
        .await
        .unwrap();
    let host_elapsed = started.elapsed();

    let root = tempfile::tempdir().unwrap();
    let settings = StorageSettings {
        filesystem: StorageFilesystemSettings {
            poll_interval: Duration::from_millis(100),
            max_entries: nz(100_000),
            event_capacity: nz(1024),
        },

        request: StorageRequestSettings {
            max_pending_queries: nz(64),
            max_active_queries: nz(4),
            max_pending_write_batches: nz(256),
            max_active_file_operations: nz(4),
            max_pending_write_bytes: nz(64 * 1024 * 1024),
        },
        response: StorageResponseSettings { batch_rows: nz(1024), batch_bytes: nz(1024 * 1024) },
    };
    let history = StoreHistory {
        store: harmos_runtime::Identifier::new("carrier-benchmark").unwrap(),
        epoch: harmos_runtime::Identifier::new("first").unwrap(),
    };
    let primary = JsonStorage::new(JsonStorageSettings { root: root.path().to_owned(), max_entry_bytes: nz(16 * 1024 * 1024) }, history);
    let backends = primary
        .into_backends(state::StorageStateSettings { max_entry_bytes: nz(16 * 1024 * 1024), max_checkpoint_bytes: nz(16 * 1024 * 1024) });
    let storage: StorageManager = StorageManager::new(backends, settings).unwrap();
    harmos_runtime::storage::operations::register(&runtime.operations, "storage", Lanes::Open, Arc::new(storage.clone())).unwrap();
    (runtime, storage, root, host_elapsed)
}

fn plugin_settings() -> harmos_runtime::config::PluginSettings {
    harmos_runtime::config::PluginSettings {
        configuration: b"null".to_vec(),
        requires: Vec::new(),
        host_operations: vec![harmos_runtime::Identifier::new("storage.hash_file").unwrap()],
        initialization_timeout: Duration::from_secs(30),
        shutdown_timeout: Duration::from_secs(30),
        encoding_memory_bytes: nz(4 * 1024 * 1024),
        restart: harmos_runtime::config::PluginRestartSettings {
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
