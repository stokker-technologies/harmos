/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::{Data, Recovery, history};
use harmos_runtime::storage::state::RecoverySnapshot;
use harmos_runtime::storage::{StorageBackends, StorageManager, StorageSettings};
use std::sync::Arc;

/* === Structs === */

pub(crate) struct Store {
    pub(crate) data: Arc<Data>,
    pub(crate) recovery: Arc<Recovery>,
}

/* === Implementations === */

impl Store {
    pub(crate) fn new() -> Self {
        Self {
            data: Arc::new(Data::new(history())),
            recovery: Arc::new(Recovery::new(history())),
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
