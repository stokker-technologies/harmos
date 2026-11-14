/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::Backend;
use super::RecoveryBackend;
use super::history;
use harmos_runtime::storage::state::RecoverySnapshot;
use harmos_runtime::storage::{StorageBackends, StorageManager, StorageSettings};
use std::sync::Arc;

/* === Structs === */

pub(crate) struct Store {
    pub(crate) data: Arc<Backend>,
    pub(crate) recovery: Arc<RecoveryBackend>,
}

/* === Implementations === */

impl Store {
    pub(crate) fn new() -> Self {
        Self {
            data: Arc::new(Backend::new(history())),
            recovery: Arc::new(RecoveryBackend::new(history())),
        }
    }
    pub(crate) fn manager(&self, config: StorageSettings) -> StorageManager {
        StorageManager::new(
            StorageBackends {
                primary: self.data.clone(),
                state: self.recovery.clone(),
            },
            config,
        )
        .unwrap()
    }
    pub(crate) fn snapshot(&self) -> RecoverySnapshot {
        self.recovery.snapshot.lock().unwrap().clone().unwrap()
    }
}
