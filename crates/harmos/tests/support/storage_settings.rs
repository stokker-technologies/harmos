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
use harmos::storage::*;
use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;
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
            reserved_control_batches: 0,
            reserved_control_bytes: 0,
        },
        response: StorageResponseSettings {
            batch_rows: nz(64),
            batch_bytes: nz(64 * 1024),
        },
        target_batch_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
        max_batch_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
        query_page: ReadLimits {
            rows: NonZeroUsize::new(64).unwrap(),
            bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        },
        query_decoded_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        query_execution: QueryLimits {
            memory_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            admission: Admission::Guaranteed,
            spill_bytes: 0,
            timeout: Duration::from_secs(5),
        },
        replication_limits: ReadLimits {
            rows: NonZeroUsize::new(64).unwrap(),
            bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        },
        replica_lag_positions: NonZeroU64::new(4096).unwrap(),
        replication_shutdown: Duration::ZERO,
        max_files: NonZeroUsize::new(8).unwrap(),
        file_session_timeout: Duration::from_secs(60),
        max_mutations: NonZeroUsize::new(1024).unwrap(),
        mutation_memory_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        mutation_retention: Duration::from_secs(300),
        view_idle_timeout: Duration::from_secs(30),
        snapshot_bytes: NonZeroUsize::new(4096).unwrap(),
        write_batch_entries: NonZeroUsize::new(64).unwrap(),
        object_prune_entries: NonZeroUsize::new(256).unwrap(),
        write_batch_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
        flush_interval: Duration::from_millis(100),
        io_timeout: Duration::from_secs(5),
        max_retries: 3,
        retry_base: Duration::from_millis(10),
        retry_cap: Duration::from_secs(1),
    }
}
/* === Private Functions === */
fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}
/* === Tests === */
