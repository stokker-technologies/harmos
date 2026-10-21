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
use std::sync::Arc;

// Third Party Imports
use harmos_runtime::Runtime;
use harmos_runtime::storage::*;

// Local Imports
use super::{Model, State, backend};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Fixture {
    pub settings: StorageSettings,
    _directory: tempfile::TempDir,
    pub storage: StorageManager,
    pub runtime: Runtime<Model>,
    pub data: Arc<backend::Data>,
}
/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Fixture {
    pub async fn new(concurrency: usize) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let data = Arc::new(backend::Data::journaled(directory.path(), backend::history()).unwrap());
        let mut settings = backend::storage_settings();
        settings.request.max_active_queries = super::nonzero(concurrency);
        settings.budget.read_memory_bytes = super::nonzero(128 * 1024 * 1024);
        settings.request.max_pending_write_bytes = super::nonzero(32 * 1024 * 1024);
        let storage = StorageManager::new(
            StorageBackends { primary: data.clone(), state: Arc::new(backend::Recovery::new(backend::history())) },
            settings.clone(),
        )
        .unwrap();
        let runtime = Runtime::<Model>::builder(State::default(), ())
            .register(super::Transactions::catalog())
            .register(super::Records::catalog())
            .runtime_with_storage(&storage)
            .await
            .unwrap();
        Self { settings, _directory: directory, storage, runtime, data }
    }

    pub async fn finish(self) {
        self.storage.close().await.unwrap();
        self.runtime.stop().await.unwrap();
    }
}
/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
