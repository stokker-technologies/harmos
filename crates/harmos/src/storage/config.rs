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
use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

// Third Party Imports

// Local Imports
use super::{QueryLimits, ReadLimits};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug)]
pub struct StorageSettings {
    pub queue_capacity_bytes: NonZeroUsize,
    pub target_batch_bytes: NonZeroUsize,
    pub max_batch_bytes: NonZeroUsize,
    pub read_memory_bytes: NonZeroUsize,
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
    pub file_chunk_bytes: NonZeroUsize,
    pub file_session_timeout: Duration,
    pub view_idle_timeout: Duration,
    pub snapshot_bytes: NonZeroUsize,
    pub queue_capacity: NonZeroUsize,
    pub write_batch_entries: NonZeroUsize,
    pub object_prune_entries: NonZeroUsize,
    pub write_batch_bytes: NonZeroUsize,
    pub flush_interval: Duration,
    pub io_timeout: Duration,
    pub max_retries: usize,
    pub retry_base: Duration,
    pub retry_cap: Duration,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            queue_capacity_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
            target_batch_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
            max_batch_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
            read_memory_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
            query_page: ReadLimits {
                rows: NonZeroUsize::new(64).unwrap(),
                bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            },
            query_page_ceiling: ReadLimits {
                rows: NonZeroUsize::new(4096).unwrap(),
                bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            },
            query_decoded_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            query_decoded_bytes_ceiling: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            query_execution: QueryLimits {
                memory_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                spill_bytes: 0,
                timeout: Duration::from_secs(5),
            },
            query_execution_ceiling: QueryLimits {
                memory_bytes: NonZeroUsize::new(16 * 1024 * 1024).unwrap(),
                spill_bytes: 64 * 1024 * 1024,
                timeout: Duration::from_secs(30),
            },
            replication_limits: ReadLimits {
                rows: NonZeroUsize::new(64).unwrap(),
                bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            },
            replica_lag_positions: NonZeroU64::new(4096).unwrap(),
            replication_shutdown: Duration::ZERO,
            max_views: NonZeroUsize::new(32).unwrap(),
            max_followers: NonZeroUsize::new(32).unwrap(),
            max_files: NonZeroUsize::new(8).unwrap(),
            file_chunk_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            file_session_timeout: Duration::from_secs(60),
            max_mutations: NonZeroUsize::new(1024).unwrap(),
            mutation_memory_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
            mutation_retention: Duration::from_secs(300),
            view_idle_timeout: Duration::from_secs(30),
            snapshot_bytes: NonZeroUsize::new(4096).unwrap(),
            queue_capacity: NonZeroUsize::new(256).unwrap(),
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
}

impl StorageSettings {
    pub(crate) fn check(&self) -> crate::Result<()> {
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
            || self.query_decoded_bytes > self.read_memory_bytes
        {
            return Err(crate::Error::InvalidInput {
                field: "query decoding",
                reason: "default decoded capacity exceeds its ceiling or shared read capacity",
            });
        }
        if self.query_execution.memory_bytes > self.read_memory_bytes {
            return Err(crate::Error::InvalidInput {
                field: "query execution",
                reason: "default working memory exceeds shared read capacity",
            });
        }
        if self.target_batch_bytes > self.max_batch_bytes
            || self.max_batch_bytes > self.queue_capacity_bytes
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
