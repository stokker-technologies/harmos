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
use std::time::{Duration, Instant};

// Third Party Imports

// Local Imports
use harmos_runtime::Runtime;
use schematic_domain::model::Snapshot;
use schematic_operations::guest::schematic::{propose, validate};

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
        runtime
            .plugin
            .register_guest(
                harmos_runtime::operations::ComponentCatalog::new()
                    .operation(schematic_operations::guest::VALIDATE_SCHEMATIC_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::PROPOSE_ROUTE_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::ANALYZE_SCHEMATIC_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::transfer::UPLOAD_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::transfer::DOWNLOAD_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::transfer::PUT_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::transfer::TAKE_OPERATION)
                    .unwrap()
                    .operation(schematic_operations::guest::transfer::HASH_OPERATION)
                    .unwrap(),
                loaded,
                plugin_settings(),
                harmos_runtime::operations::Lane::default(),
            )
            .await
            .unwrap();
        runtime.plugin.initialize().await.unwrap();
        Self {
            runtime,
            startup_elapsed: host_elapsed + started.elapsed(),
        }
    }

    pub async fn command(&self) -> harmos_runtime::operations::Response<propose::Route> {
        self.runtime
            .operation(propose::ProposeRoute {
                from: schematic_domain::model::Point { x: 0, y: 0 },
                to: schematic_domain::model::Point { x: 3, y: 4 },
            })
            .await
            .unwrap()
    }

    pub async fn payload(&self, request: validate::ValidateSchematic) -> harmos_runtime::operations::Response<validate::Validation> {
        self.runtime.operation(request).await.unwrap()
    }

    pub async fn finish(self) {
        self.runtime.shutdown().await.unwrap();
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
    validate::ValidateSchematic {
        schematic: Snapshot { symbols, connections: Vec::new() },
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
    let runtime = Runtime::<Model>::launch_with((), (), |setup| {
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            queued: nz(64),
            lifecycle_queued: nz(16),
            preparation_bytes: Some(preparation_bytes),
        });
    })
    .await
    .unwrap();
    let host_elapsed = started.elapsed();
    (runtime, host_elapsed)
}

fn plugin_settings() -> harmos_runtime::config::PluginSettings {
    harmos_runtime::config::PluginSettings {
        configuration: b"null".to_vec(),
        requires: Vec::new(),
        host_operations: Vec::new(),
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
