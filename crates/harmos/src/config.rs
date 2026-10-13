/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Externally Supplied Runtime, Storage, Transport And Tooling Settings.

/* === Modules === */

/* === Re-Exports === */
pub use harmos_contract::config::SchemaSettings;

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

// Third Party Imports

// Local Imports
use crate::storage::{QueryLimits, ReadLimits};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[cfg(feature = "grpc")]
#[derive(Clone, Copy, Debug)]
pub struct DecodeSettings {
    pub message_bytes: NonZeroUsize,
    pub retained_bytes: NonZeroUsize,
}

#[cfg(feature = "grpc")]
#[derive(Clone, Copy, Debug)]
pub struct EncodeSettings {
    pub message_bytes: NonZeroUsize,
    pub buffer_bytes: NonZeroUsize,
}

#[cfg(feature = "grpc")]
#[derive(Clone, Copy, Debug)]
pub struct GrpcServerSettings {
    pub calls: std::num::NonZeroUsize,
    pub decode: DecodeSettings,
    pub encode: EncodeSettings,
    pub input: crate::operations::StreamSettings,
    pub output: crate::operations::StreamSettings,
    pub opening_timeout: Duration,
}

#[cfg(feature = "grpc")]
#[derive(Clone, Debug)]
pub struct GrpcSettings {
    pub address: std::net::SocketAddr,
    pub operations: GrpcServerSettings,
    pub reflection: bool,
    pub web: bool,
    // Exact browser origins allowed through the gRPC-Web listener.
    // An empty list emits no CORS headers.
    pub allowed_origins: Vec<crate::grpc::Origin>,
    pub tls: Option<tonic::transport::ServerTlsConfig>,
    pub connections: NonZeroUsize,
    pub calls_per_connection: NonZeroUsize,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
}

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
    // Resume Retention For Disconnected/Recovered Producers And Inactivity
    // Retention For Token-Only Producers Without A Retained Native Handle.
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

#[derive(Clone, Copy, Debug)]
pub struct OperationSettings {
    pub queued: NonZeroUsize,
    pub lifecycle_queued: NonZeroUsize,
    pub preparation_bytes: Option<NonZeroUsize>,
}

#[cfg(feature = "plugin")]
#[derive(Clone, Debug)]
pub struct PluginSettings {
    pub configuration: Vec<u8>,
    pub requires: Vec<harmos_contract::Requirement>,
    pub host_operations: Vec<crate::Identifier>,
    pub initialization_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub encoding_memory_bytes: NonZeroUsize,
    pub restart: PluginRestartSettings,
}

#[cfg(feature = "plugin")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluginRestartSettings {
    // Automatic Restarts Allowed Per Explicit Activation Cycle.
    pub retries: u32,
    pub backoff_base: Duration,
    pub backoff_cap: Duration,
}

#[cfg(feature = "plugin-sidecar")]
#[derive(Clone, Debug)]
pub struct PluginSidecarSettings {
    pub plugin: PluginSettings,
    pub calls: NonZeroUsize,
}

#[cfg(feature = "plugin-guest")]
#[derive(Clone, Copy, Debug)]
pub struct PluginGuestSettings {
    pub component_bytes: NonZeroUsize,
    pub memory_bytes: NonZeroUsize,
    pub fuel: NonZeroU64,
    pub memories: NonZeroUsize,
    pub table_elements: NonZeroUsize,
    pub tables: NonZeroUsize,
    pub instances: NonZeroUsize,
}

#[cfg(feature = "packaging")]
#[derive(Clone, Debug)]
pub struct BuildSettings {
    pub backend: BuildBackend,
    pub manifest: std::path::PathBuf,
    pub packages: Vec<String>,
    pub target: Option<String>,
    pub target_directory: Option<std::path::PathBuf>,
    pub release: bool,
    pub timings: bool,
    pub features: Vec<String>,
    pub default_features: bool,
}

#[cfg(feature = "packaging")]
#[derive(Clone, Copy, Debug)]
pub struct DevSettings {
    pub debounce: Duration,
}

/* === Enums === */
#[cfg(feature = "packaging")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildBackend {
    Sidecar,
    Guest,
}

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "plugin")]
impl Default for PluginRestartSettings {
    fn default() -> Self {
        Self {
            retries: 0,
            backoff_base: Duration::from_millis(100),
            backoff_cap: Duration::from_secs(5),
        }
    }
}

#[cfg(feature = "plugin")]
impl PluginRestartSettings {
    pub fn validate(&self) -> Result<()> {
        if self.backoff_base.is_zero()
            || self.backoff_cap < self.backoff_base
            || std::time::Instant::now()
                .checked_add(self.backoff_cap)
                .is_none()
        {
            return Err(Error::InvalidInput {
                field: "plugin restart backoff",
                reason: "requires positive representable bounds with base no greater than cap",
            });
        }
        Ok(())
    }
}

#[cfg(feature = "plugin-sidecar")]
impl Default for PluginSidecarSettings {
    fn default() -> Self {
        Self {
            plugin: PluginSettings::default(),
            calls: NonZeroUsize::new(4).unwrap(),
        }
    }
}

#[cfg(feature = "plugin")]
impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            configuration: b"null".to_vec(),
            requires: Vec::new(),
            host_operations: Vec::new(),
            initialization_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(30),
            encoding_memory_bytes: NonZeroUsize::new(4096).unwrap(),
            restart: PluginRestartSettings::default(),
        }
    }
}

#[cfg(feature = "plugin")]
impl PluginSettings {
    pub fn validate(&self) -> Result<()> {
        self.restart.validate()?;
        for timeout in [self.initialization_timeout, self.shutdown_timeout] {
            if timeout.is_zero() || std::time::Instant::now().checked_add(timeout).is_none() {
                return Err(Error::InvalidInput {
                    field: "plugin lifecycle timeout",
                    reason: "must be positive and representable",
                });
            }
        }
        let grants: std::collections::BTreeSet<_> = self.host_operations.iter().collect();
        if grants.len() != self.host_operations.len() {
            return Err(Error::InvalidInput {
                field: "plugin host operations",
                reason: "grants must be unique",
            });
        }
        Ok(())
    }
}

#[cfg(feature = "plugin-guest")]
impl Default for PluginGuestSettings {
    fn default() -> Self {
        Self {
            component_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
            memory_bytes: NonZeroUsize::new(64 * 1024 * 1024).unwrap(),
            fuel: NonZeroU64::new(10_000_000).unwrap(),
            memories: NonZeroUsize::new(1).unwrap(),
            table_elements: NonZeroUsize::new(65_536).unwrap(),
            tables: NonZeroUsize::new(8).unwrap(),
            instances: NonZeroUsize::new(8).unwrap(),
        }
    }
}
#[cfg(feature = "grpc")]
impl Default for GrpcServerSettings {
    fn default() -> Self {
        let bytes = std::num::NonZeroUsize::new(16 * 1024 * 1024).unwrap();
        let message = std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap();
        let items = std::num::NonZeroUsize::new(16).unwrap();
        Self {
            calls: std::num::NonZeroUsize::new(64).unwrap(),
            decode: DecodeSettings {
                message_bytes: message,
                retained_bytes: bytes,
            },
            encode: EncodeSettings {
                message_bytes: message,
                buffer_bytes: std::num::NonZeroUsize::new(8192).unwrap(),
            },
            input: crate::operations::StreamSettings { items, bytes },
            output: crate::operations::StreamSettings { items, bytes },
            opening_timeout: Duration::from_secs(5),
        }
    }
}

#[cfg(feature = "grpc")]
impl Default for GrpcSettings {
    fn default() -> Self {
        Self {
            address: ([127, 0, 0, 1], 0).into(),
            operations: GrpcServerSettings::default(),
            reflection: false,
            web: false,
            allowed_origins: Vec::new(),

            tls: None,
            connections: NonZeroUsize::new(64).unwrap(),
            calls_per_connection: NonZeroUsize::new(32).unwrap(),
            request_timeout: Duration::from_secs(60),
            shutdown_timeout: Duration::from_secs(10),
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

impl OperationSettings {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.queued.get() > tokio::sync::Semaphore::MAX_PERMITS
            || self.lifecycle_queued.get() > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(Error::InvalidInput {
                field: "operation queue capacity",
                reason: "capacity exceeds the semaphore claim limit",
            });
        }
        if self.preparation_bytes.is_some_and(|bytes| {
            bytes.get() > tokio::sync::Semaphore::MAX_PERMITS || bytes.get() > u32::MAX as usize
        }) {
            return Err(Error::InvalidInput {
                field: "operation preparation bytes",
                reason: "budget exceeds the semaphore byte-claim limit",
            });
        }
        Ok(())
    }
}

impl Default for OperationSettings {
    fn default() -> Self {
        Self {
            queued: NonZeroUsize::new(256).expect("nonzero ordinary admission bound"),
            lifecycle_queued: NonZeroUsize::new(256).expect("nonzero lifecycle admission bound"),
            preparation_bytes: None,
        }
    }
}

#[cfg(feature = "packaging")]
impl Default for BuildSettings {
    fn default() -> Self {
        Self {
            backend: BuildBackend::Sidecar,
            manifest: "Cargo.toml".into(),
            packages: Vec::new(),
            target: None,
            target_directory: None,
            release: false,
            timings: false,
            features: Vec::new(),
            default_features: true,
        }
    }
}

#[cfg(feature = "packaging")]
impl Default for DevSettings {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(100),
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
