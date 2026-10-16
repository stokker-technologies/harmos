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
use super::{QueryLimits, ReadLimits};
use serde::{Deserialize, Serialize};
use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct StorageSettings {
    pub budget: StorageBudget,
    pub request: StorageRequestSettings,
    pub response: StorageResponseSettings,
    pub target_batch_bytes: NonZeroUsize,
    pub max_batch_bytes: NonZeroUsize,
    pub query_page: ReadLimits,
    pub query_page_ceiling: ReadLimits,
    pub query_decoded_bytes: NonZeroUsize,
    pub query_decoded_bytes_ceiling: NonZeroUsize,
    pub query_execution: QueryLimits,
    pub query_execution_ceiling: QueryLimits,
    pub replication_limits: ReadLimits,
    pub replica_lag_positions: NonZeroU64,
    pub replication_shutdown: Duration,
    pub max_views: NonZeroUsize,
    pub max_followers: NonZeroUsize,
    pub max_files: NonZeroUsize,
    pub max_mutations: NonZeroUsize,
    pub mutation_memory_bytes: NonZeroUsize,
    pub mutation_retention: Duration,
    // Resume Retention For Disconnected/Recovered Producers And Inactivity
    // Retention For Token-Only Producers Without A Retained Native Handle.
    pub file_session_timeout: Duration,
    pub view_idle_timeout: Duration,
    pub snapshot_bytes: NonZeroUsize,
    pub write_batch_entries: NonZeroUsize,
    pub object_prune_entries: NonZeroUsize,
    pub write_batch_bytes: NonZeroUsize,
    pub flush_interval: Duration,
    pub io_timeout: Duration,
    pub max_retries: usize,
    pub retry_base: Duration,
    pub retry_cap: Duration,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StorageBudget {
    pub read_memory_bytes: NonZeroUsize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StorageRequestSettings {
    pub max_pending_queries: NonZeroUsize,
    pub max_active_queries: NonZeroUsize,
    pub max_pending_write_batches: NonZeroUsize,
    pub max_pending_write_bytes: NonZeroUsize,
    pub reserved_control_batches: usize,
    pub reserved_control_bytes: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StorageResponseSettings {
    pub batch_rows: NonZeroUsize,
    pub batch_bytes: NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl StorageSettings {
    pub fn validate(&self) -> crate::Result<()> {
        if self.request.max_active_queries.get() > tokio::sync::Semaphore::MAX_PERMITS
            || self.request.max_pending_queries.get() > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(crate::Error::InvalidInput {
                field: "storage query admission",
                reason: "query counts exceed semaphore capacity",
            });
        }
        if self.request.reserved_control_batches >= self.request.max_pending_write_batches.get()
            || self.request.reserved_control_bytes >= self.request.max_pending_write_bytes.get()
        {
            return Err(crate::Error::InvalidInput {
                field: "storage control reserve",
                reason: "reserved capacity must be smaller than total write capacity",
            });
        }

        if self.mutation_retention.is_zero()
            || tokio::time::Instant::now()
                .checked_add(self.mutation_retention)
                .is_none()
            || std::time::SystemTime::now()
                .checked_add(self.mutation_retention)
                .is_none()
        {
            return Err(crate::Error::InvalidInput {
                field: "mutation retention",
                reason: "must be positive and representable",
            });
        }
        self.query_page_ceiling
            .check(self.query_page.rows.get(), self.query_page.bytes.get())?;
        self.query_execution
            .check_ceiling(self.query_execution_ceiling)?;
        if self.query_decoded_bytes > self.query_decoded_bytes_ceiling
            || self.query_decoded_bytes > self.budget.read_memory_bytes
        {
            return Err(crate::Error::InvalidInput {
                field: "query decoding",
                reason: "default decoded capacity exceeds its ceiling or shared read capacity",
            });
        }
        if self.query_execution.memory_bytes > self.budget.read_memory_bytes {
            return Err(crate::Error::InvalidInput {
                field: "query execution",
                reason: "default working memory exceeds shared read capacity",
            });
        }
        if self.target_batch_bytes > self.max_batch_bytes
            || self.max_batch_bytes > self.request.max_pending_write_bytes
        {
            return Err(crate::Error::InvalidInput {
                field: "storage policy",
                reason: "require target_batch_bytes <= max_batch_bytes <= queue_capacity_bytes",
            });
        }
        if self.file_session_timeout.is_zero()
            || std::time::Instant::now()
                .checked_add(self.file_session_timeout)
                .is_none()
            || self.view_idle_timeout.is_zero()
            || self.flush_interval.is_zero()
            || self.io_timeout.is_zero()
            || self.retry_base.is_zero()
            || self.retry_cap < self.retry_base
        {
            return Err(crate::Error::InvalidInput {
                field: "storage policy",
                reason: "invalid persistence or retry timing",
            });
        }
        Ok(())
    }
}

impl StorageSettings {
    pub(crate) fn check(&self) -> crate::Result<()> {
        self.validate()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
