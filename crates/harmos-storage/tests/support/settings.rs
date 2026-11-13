/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use harmos_storage::*;

// Local Imports

/* === Public Functions === */

pub fn storage_settings() -> StorageSettings {
    StorageSettings {
        transfer: Default::default(),
        policy: Default::default(),
        filesystem: harmos_storage::StorageFilesystemSettings {
            poll_interval: std::time::Duration::from_millis(100),
            max_entries: std::num::NonZeroUsize::new(100_000).unwrap(),
            event_capacity: std::num::NonZeroUsize::new(1024).unwrap(),
        },

        request: StorageRequestSettings {
            max_file_productions: std::num::NonZeroUsize::new(256).unwrap(),
            max_mutation_receipts: std::num::NonZeroUsize::new(1024).unwrap(),
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
    }
}
/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
