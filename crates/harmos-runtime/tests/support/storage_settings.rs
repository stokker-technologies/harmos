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
use std::num::NonZeroUsize;

// Third Party Imports
use harmos_runtime::storage::*;

// Local Imports

/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
/* === Enums === */
/* === Traits === */
/* === Implementations === */
/* === Public Functions === */
pub fn storage_settings() -> StorageSettings {
    StorageSettings {
        budget: StorageBudget {
            read_memory_bytes: nz(16 * 1024 * 1024),
        },
        request: StorageRequestSettings {
            max_pending_queries: nz(64),
            max_active_queries: nz(4),
            max_pending_write_batches: nz(256),
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
/* === Tests === */
