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
pub use harmos_domain::config::*;

/* === Imports === */

// Standard Library Imports
#[cfg(feature = "plugin-guest")]
use std::num::NonZeroU64;
use std::num::NonZeroUsize;
#[cfg(any(feature = "grpc", feature = "plugin", feature = "packaging"))]
use std::time::Duration;

// Third Party Imports

// Local Imports
pub use crate::storage::config::{
    StorageBudget, StorageRequestSettings, StorageResponseSettings, StorageSettings,
};
pub use crate::storage::state::StorageStateSettings;
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
    pub requires: Vec<harmos_domain::Requirement>,
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
