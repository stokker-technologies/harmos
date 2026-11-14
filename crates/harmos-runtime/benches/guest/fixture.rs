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

#[path = "../../../harmos-storage/tests/support/storage.rs"]
#[allow(dead_code, reason = "The Carrier Fixture Needs Only the Native File Callee's Backends")]
mod backend;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
#[cfg(feature = "plugin-guest")]
use std::num::NonZeroU64;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Third Party Imports

// Local Imports
use harmos_runtime::Runtime;
use prost::Message as _;
use schematic_operations::protocol::{Point, ProposeRoute, Route, Snapshot, ValidateSchematic, Validation};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Model;

pub struct Fixture {
    pub runtime: Runtime<Model>,
    pub startup_elapsed: Duration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::io::Error;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _setup: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }
}

impl harmos_runtime::Application for Model {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}

impl Fixture {
    #[cfg(feature = "plugin-guest")]
    #[allow(dead_code, reason = "Each Carrier Benchmark Selects Its Matching Constructor")]
    pub async fn guest() -> Self {
        let (runtime, host_elapsed) = runtime().await;
        let started = Instant::now();
        let bytes = std::fs::read(artifact("schematic-guest")).unwrap();
        let settings = harmos_runtime::config::PluginGuestSettings {
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
        let host = harmos_runtime::guest::Host::with_settings(&runtime, settings).unwrap();
        let loaded = host.load(&bytes).unwrap();
        runtime.plugin.register_guest(loaded, plugin_settings(), harmos_runtime::operations::Lane::default()).await.unwrap();
        runtime.plugin.initialize().await.unwrap();
        Self {
            runtime,
            startup_elapsed: host_elapsed + started.elapsed(),
        }
    }

    pub async fn command(&self) -> Route {
        let request = ProposeRoute {
            from: Some(Point { x: 0, y: 0 }),
            to: Some(Point { x: 3, y: 4 }),
        };
        let payload = self.runtime.operations.call_encoded("schematic.route", request.encode_to_vec()).await.unwrap();
        Route::decode(payload.as_slice()).unwrap()
    }
    pub async fn payload(&self, request: ValidateSchematic) -> Validation {
        let payload = self.runtime.operations.call_encoded("schematic.validate", request.encode_to_vec()).await.unwrap();
        Validation::decode(payload.as_slice()).unwrap()
    }

    pub async fn finish(self) {
        self.runtime.shutdown().await.unwrap();
    }
}

/* === Public Functions === */

pub fn verify_route(answer: &Route) {
    assert_eq!(answer.points, [Point { x: 0, y: 0 }, Point { x: 3, y: 0 }, Point { x: 3, y: 4 },]);
}

pub fn payload(symbols: usize) -> ValidateSchematic {
    let symbols = (0..symbols).map(|value| value as u64).collect();
    ValidateSchematic {
        schematic: Some(Snapshot { symbols, connections: Vec::new() }),
    }
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

async fn runtime() -> (Runtime<Model>, Duration) {
    let started = Instant::now();
    let preparation_bytes = nz(64 * 1024 * 1024);
    let storage = harmos_runtime::storage::StorageManager::new(
        harmos_runtime::storage::StorageBackends {
            primary: std::sync::Arc::new(backend::Backend::new(backend::history())),
            state: std::sync::Arc::new(backend::RecoveryBackend::new(backend::history())),
        },
        backend::storage_settings(),
    )
    .unwrap();
    let runtime = Runtime::<Model>::launch_with((), (), |setup| {
        setup.resource::<harmos_runtime::storage::StorageManager>(storage.clone());
        setup.catalog(harmos_operations::storage_catalog(), harmos_runtime::operations::Lane::default());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            queued: nz(64),
            lifecycle_queued: nz(16),
            preparation_bytes: Some(preparation_bytes),
        });
    })
    .await
    .unwrap();
    runtime.start_storage(storage).await.unwrap();
    let host_elapsed = started.elapsed();
    (runtime, host_elapsed)
}

fn plugin_settings() -> harmos_runtime::config::PluginSettings {
    harmos_runtime::config::PluginSettings {
        configuration: b"null".to_vec(),
        requires: Vec::new(),
        grants: Vec::new(),
        host_operations: ["schematic.v1.Files/Hash", "harmos.plugin.host.v1.Settings/List"]
            .into_iter()
            .map(|id| harmos_runtime::Identifier::new(id).unwrap())
            .collect(),
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
