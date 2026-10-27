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
pub(super) const TARGET_BATCH_BYTES: usize = 128 * 1024;
pub(super) const OBJECT_PRUNE_ENTRIES: NonZeroUsize = NonZeroUsize::new(256).unwrap();
pub(super) const SNAPSHOT_BYTES: usize = 1024 * 1024;

// Backend I/O And Transient Persistence Retry.
pub(super) const IO_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const MAX_RETRIES: usize = 3;
pub(super) const RETRY_BASE: Duration = Duration::from_millis(10);
pub(super) const RETRY_CAP: Duration = Duration::from_secs(1);

// Producer Sessions And Idempotent Mutation Receipts.
pub(super) const MAX_FILES: NonZeroUsize = NonZeroUsize::new(256).unwrap();
pub(super) const FILE_SESSION_TIMEOUT: Duration = Duration::from_secs(60);
pub(super) const MAX_MUTATIONS: NonZeroUsize = NonZeroUsize::new(1024).unwrap();
pub(super) const MUTATION_RETENTION: Duration = Duration::from_secs(300);
// The Receipt Ledger Takes This Share Of The Read Budget That Checkpoints Copy It Into.
const MUTATION_BUDGET_SHARE: usize = 8;

// Internal Scan Pages; Response Sizing Belongs To StorageResponseSettings.
pub(super) const SCAN_ROWS: NonZeroUsize = NonZeroUsize::new(1024).unwrap();
pub(super) const SCAN_BYTES: usize = 1024 * 1024;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageSettings {
    pub budget: StorageBudget,
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
pub struct StorageBudget {
    pub read_memory_bytes: NonZeroUsize,
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
        if self.response.batch_bytes.get() > self.budget.read_memory_bytes.get() / 2 {
            return Err(crate::Error::InvalidInput {
                field: "storage response",
                reason: "response batches cannot exceed half the shared read budget",
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

    // Receipt Ledger Bound: A Fixed Share Of The Shared Read Budget.
    pub(super) fn mutation_bytes(&self) -> NonZeroUsize {
        NonZeroUsize::new(self.budget.read_memory_bytes.get() / MUTATION_BUDGET_SHARE).unwrap_or(NonZeroUsize::MIN)
    }

    // Scan Page Bytes, Small Enough That Several Active Queries Share The Budget.
    pub(super) fn scan_bytes(&self) -> NonZeroUsize {
        let shared = self.budget.read_memory_bytes.get() / self.request.max_active_queries.get().saturating_mul(4).max(1);
        NonZeroUsize::new(SCAN_BYTES.min(shared).max(1)).unwrap()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::{StorageBudget, StorageRequestSettings, StorageResponseSettings, StorageSettings};
    use std::num::NonZeroUsize;

    fn settings() -> StorageSettings {
        StorageSettings {
            filesystem: crate::config::StorageFilesystemSettings {
                poll_interval: std::time::Duration::from_millis(100),
                max_entries: std::num::NonZeroUsize::new(100_000).unwrap(),
                event_capacity: std::num::NonZeroUsize::new(1024).unwrap(),
            },
            budget: StorageBudget {
                read_memory_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
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
    fn response_fits_budget() {
        let mut settings = settings();
        settings.validate().unwrap();

        settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() + 1).unwrap();
        assert!(settings.validate().is_err());
        settings.response.batch_bytes = settings.budget.read_memory_bytes;
        assert!(settings.validate().is_err());

        settings.response.batch_bytes = NonZeroUsize::new(settings.budget.read_memory_bytes.get() / 2).unwrap();
        settings.validate().unwrap();
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
