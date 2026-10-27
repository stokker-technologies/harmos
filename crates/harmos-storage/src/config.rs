/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Explicit Storage Configuration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::time::Duration;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

// Persistence Cadence And Batching Are Private Storage Algorithm Choices.
pub(super) const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
pub(super) const MAX_BATCH_BYTES: usize = 16 * 1024 * 1024;
pub(super) const TARGET_BATCH_BYTES: usize = 1024 * 1024;
pub(super) const OBJECT_PRUNE_ENTRIES: NonZeroUsize = NonZeroUsize::new(256).unwrap();
pub(super) const SNAPSHOT_BYTES: usize = 1024 * 1024;

// Backend I/O And Transient Persistence Retry.
pub(super) const IO_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const RECOVERY_TIMEOUT: Duration = Duration::from_secs(300);
pub(super) const MAX_RETRIES: usize = 3;
pub(super) const RETRY_BASE: Duration = Duration::from_millis(10);
pub(super) const RETRY_CAP: Duration = Duration::from_secs(1);

// Producer Sessions And Idempotent Mutation Receipts.
pub(super) const MAX_FILES: NonZeroUsize = NonZeroUsize::new(256).unwrap();
pub(super) const FILE_SESSION_TIMEOUT: Duration = Duration::from_secs(60);
pub(super) const MAX_MUTATIONS: NonZeroUsize = NonZeroUsize::new(1024).unwrap();
pub(super) const MUTATION_RETENTION: Duration = Duration::from_secs(300);
// Internal Scan Pages Amortize Backend Calls On Desktop Storage: 16,384 Rows / 8 MiB.
// Response Sizing And Concurrent Query Counts Remain Explicit Consumer Settings.
// Query Reductions Cache At Most 32 MiB Of Arrow Pages And 32 MiB Of Coordinates,
// Then Spill To Disk Within The Existing 128 MiB Algorithm Workspace Target.
pub(super) const SCAN_ROWS: NonZeroUsize = NonZeroUsize::new(16_384).unwrap();
pub(super) const SCAN_BYTES: usize = 8 * 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageSettings {
    pub request: StorageRequestSettings,
    pub response: StorageResponseSettings,
    pub filesystem: StorageFilesystemSettings,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageFilesystemSettings {
    pub poll_interval: Duration,
    pub max_entries: NonZeroUsize,
    pub event_capacity: NonZeroUsize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageRequestSettings {
    pub max_pending_queries: NonZeroUsize,
    pub max_active_queries: NonZeroUsize,
    pub max_pending_write_batches: NonZeroUsize,
    pub max_active_file_operations: NonZeroUsize,
    pub max_pending_write_bytes: NonZeroUsize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageResponseSettings {
    pub batch_rows: NonZeroUsize,
    pub batch_bytes: NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StorageSettings {
    pub fn validate(&self) -> crate::Result<()> {
        if self.filesystem.poll_interval.is_zero() || self.filesystem.event_capacity.get() > tokio::sync::Semaphore::MAX_PERMITS {
            return Err(crate::Error::InvalidInput {
                field: "storage filesystem",
                reason: "poll interval must be positive and event capacity bounded",
            });
        }

        if self.request.max_active_queries.get() > tokio::sync::Semaphore::MAX_PERMITS
            || self.request.max_pending_queries.get() > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(crate::Error::InvalidInput {
                field: "storage query admission",
                reason: "query counts exceed semaphore capacity",
            });
        }
        Ok(())
    }

    // Largest Atomic Batch: The Private Ceiling Within The Pending Write Budget.
    pub fn max_batch_bytes(&self) -> NonZeroUsize {
        self.request.max_pending_write_bytes.min(NonZeroUsize::new(MAX_BATCH_BYTES).unwrap())
    }

    pub(super) fn target_batch_bytes(&self) -> usize {
        TARGET_BATCH_BYTES.min(self.max_batch_bytes().get())
    }

    pub(super) fn scan_bytes(&self) -> NonZeroUsize {
        NonZeroUsize::new(SCAN_BYTES).unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{StorageRequestSettings, StorageResponseSettings, StorageSettings};
    use std::num::NonZeroUsize;

    fn settings() -> StorageSettings {
        StorageSettings {
            filesystem: crate::config::StorageFilesystemSettings {
                poll_interval: std::time::Duration::from_millis(100),
                max_entries: std::num::NonZeroUsize::new(100_000).unwrap(),
                event_capacity: std::num::NonZeroUsize::new(1024).unwrap(),
            },

            request: StorageRequestSettings {
                max_pending_queries: NonZeroUsize::new(64).unwrap(),
                max_active_queries: NonZeroUsize::new(4).unwrap(),
                max_pending_write_batches: NonZeroUsize::new(256).unwrap(),
                max_active_file_operations: NonZeroUsize::MIN,
                max_pending_write_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
            },
            response: StorageResponseSettings {
                batch_rows: NonZeroUsize::new(64).unwrap(),
                batch_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            },
        }
    }

    #[test]
    fn query_counts_bounded() {
        let mut settings = settings();
        settings.request.max_active_queries = NonZeroUsize::new(tokio::sync::Semaphore::MAX_PERMITS + 1).unwrap();
        assert!(settings.validate().is_err());

        settings.request.max_active_queries = NonZeroUsize::MIN;
        settings.request.max_pending_queries = NonZeroUsize::MIN;
        settings.validate().unwrap();
    }
}
