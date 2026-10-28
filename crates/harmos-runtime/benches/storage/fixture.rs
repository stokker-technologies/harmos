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

// Third Party Imports
use harmos_runtime::Runtime;
use harmos_runtime::storage::*;
use harmos_storage::json::{JsonStorage, JsonStorageSettings};

// Local Imports
use super::{Model, State};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Fixture {
    pub settings: StorageSettings,
    _directory: tempfile::TempDir,
    pub storage: StorageManager,
    pub runtime: Runtime<Model>,
}
/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Fixture {
    pub async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let settings = settings();
        let backends = backends(directory.path());
        let storage = StorageManager::new(backends, settings.clone()).unwrap();
        let runtime = Runtime::<Model>::launch_with(State::default(), (), |setup| {
            setup.resource::<StorageManager>(storage.clone());
            setup.settings().register(super::Transactions::catalog()).register(super::Records::catalog()).storage(&storage);
        })
        .await
        .unwrap();
        Self {
            settings,
            _directory: directory,
            storage,
            runtime,
        }
    }

    pub async fn finish(self) {
        self.runtime.shutdown().await.unwrap();
    }
}
/* === Public Functions === */

pub fn settings() -> StorageSettings {
    StorageSettings {
        filesystem: harmos_storage::StorageFilesystemSettings {
            poll_interval: std::time::Duration::from_millis(100),
            max_entries: std::num::NonZeroUsize::new(100_000).unwrap(),
            event_capacity: std::num::NonZeroUsize::new(1024).unwrap(),
        },

        request: StorageRequestSettings {
            max_active_queries: super::nonzero(4),
            max_active_file_operations: super::nonzero(4),
            max_pending_queries: super::nonzero(64),
            max_pending_write_batches: super::nonzero(256),
            max_pending_write_bytes: super::nonzero(64 * 1024 * 1024),
        },
        response: StorageResponseSettings {
            batch_rows: super::nonzero(4096),
            batch_bytes: super::nonzero(1024 * 1024),
        },
    }
}

pub fn backends(root: &std::path::Path) -> StorageBackends {
    let history = StoreHistory {
        store: harmos_runtime::Identifier::new("benchmark-store").unwrap(),
        epoch: harmos_runtime::Identifier::new("benchmark-epoch").unwrap(),
    };
    let settings = JsonStorageSettings {
        root: root.to_owned(),
        max_entry_bytes: super::nonzero(16 * 1024 * 1024),
    };
    JsonStorage::new(settings, history).into_backends(harmos_storage::state::StorageStateSettings {
        max_entry_bytes: super::nonzero(16 * 1024 * 1024),
        max_checkpoint_bytes: super::nonzero(64 * 1024 * 1024),
    })
}

/* === Private Functions === */

/* === Tests === */
