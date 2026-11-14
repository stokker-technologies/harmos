/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use harmos_runtime::plugin::grpc::PlacementSettings;
use harmos_runtime::plugin::manager::PluginManagerSettings;
use harmos_runtime::storage::config::{StorageFilesystemSettings, StorageRequestSettings, StorageResponseSettings, StorageSettings};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Settings {
    pub journal_sync_entries: NonZeroUsize,
    recordings: PathBuf,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct LaunchConfig {
    pub document: crate::document::Document,
    pub settings: Settings,

    #[serde(default)]
    pub plugins: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Settings {
    pub fn new(recordings: impl Into<PathBuf>) -> Self {
        Self {
            journal_sync_entries: NonZeroUsize::new(1).unwrap(),
            recordings: recordings.into(),
        }
    }

    pub fn recordings(&self) -> &Path {
        &self.recordings
    }

    pub fn storage(&self) -> StorageSettings {
        StorageSettings {
            transfer: Default::default(),
            policy: Default::default(),
            request: StorageRequestSettings {
                max_file_productions: NonZeroUsize::new(16).unwrap(),
                max_mutation_receipts: NonZeroUsize::new(1024).unwrap(),
                max_pending_queries: NonZeroUsize::new(8).unwrap(),
                max_active_queries: NonZeroUsize::new(4).unwrap(),
                max_pending_write_batches: NonZeroUsize::new(8).unwrap(),
                max_active_file_operations: NonZeroUsize::new(4).unwrap(),
                max_write_batch_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
            },
            response: StorageResponseSettings {
                batch_rows: NonZeroUsize::new(64).unwrap(),
                batch_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            },
            filesystem: StorageFilesystemSettings {
                poll_interval: Duration::from_secs(1),
                max_entries: NonZeroUsize::new(256).unwrap(),
                event_capacity: NonZeroUsize::new(64).unwrap(),
            },
        }
    }

    pub fn plugins(&self, directory: PathBuf) -> PluginManagerSettings {
        PluginManagerSettings {
            namespace: "schematic-plugins".into(),
            directory,
            target: if cfg!(target_os = "windows") { "x86_64-pc-windows-msvc" } else { "x86_64-unknown-linux-gnu" }.into(),
            placement: PlacementSettings {
                connect_timeout: Duration::from_secs(10),
                reconnect_delay: Duration::from_millis(100),
                stop_grace: Duration::from_secs(10),
                stream_window: NonZeroUsize::new(16).unwrap(),
            },
            bundle: Default::default(),
            entries: NonZeroUsize::new(16).unwrap(),
            create_receipts: NonZeroUsize::new(1024).unwrap(),
            registry_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
            command_capacity: NonZeroUsize::new(16).unwrap(),
            retained_cuts: NonZeroUsize::new(64).unwrap(),
            watch_capacity: NonZeroUsize::new(64).unwrap(),
            response_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
