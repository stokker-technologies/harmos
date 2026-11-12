/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod answer;
pub mod artifacts;
pub mod catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub mod clock;
pub mod config;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod error;
pub mod execution;
pub mod failure;
pub mod host;
pub mod inventory;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub mod lifecycle;
pub mod listeners;
pub mod model;
pub mod observers;
pub mod operations;
pub mod plugin;
mod refusal;
mod registration;
pub mod storage;
pub mod streams;

/* === Re-Exports === */

pub use artifacts::{KINDS, ScanError, fragments};
pub use config::{ConfigError, decode_config, encode_config, validate_config};
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use error::Error;
pub use failure::Failure;
pub use harmos_domain::*;
pub use harmos_protocol::policy::{Backoff, Jitter, OperationPolicy, RetryPolicy};
pub use harmos_protocol::service::{RestartPolicy, ServicePolicy};
pub use harmos_sdk_derive::Request;
pub use inventory::{StreamDeclaration, publications::StreamSpec};
pub use operations::{OperationDefinition, OperationSpec, RequestShape};
pub use refusal::{FAILED, RESERVED, Refusal, UNKNOWN, UNREADABLE, reserved};
pub use registration::Requirement;
pub use registration::{CATALOG_EXPORT_FLAG, Registrar, Registration, RegistrationError};

#[cfg(feature = "lifecycle")]
pub use crate::execution::{Context, ExecutionContext, Operation, Resource, ResourceError, Response};
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use host::host_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use lifecycle::lifecycle_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use storage::storage_catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use streams::streams_catalog;

/* === Imports === */

// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
extern crate self as harmos_operations;

// Local Imports
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
use crate::execution::__private::Catalog;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub fn builtins_catalog() -> crate::execution::operations::Catalog {
    builtins_catalog_batch().named("harmos.builtins")
}

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub fn builtins_catalog_batch() -> Catalog {
    let mut batch = host_catalog();
    batch.operations.extend(lifecycle_catalog().operations);
    batch.operations.extend(storage_catalog().operations);
    batch.operations.extend(streams_catalog().operations);
    batch
}

/* === Public Functions === */

/* === Private Functions === */

#[doc(hidden)]
pub mod __private {
    /* === Modules === */

    /* === Re-Exports === */

    pub use crate::answer::Answer;
    pub trait CarrierContext {
        const FACILITY: &'static str;
    }
    pub use crate::inventory as PluginInventory;
    pub use crate::plugin;
    pub use crate::plugin::schema_markers as PluginSchemaMarkers;
    pub use crate::plugin::{
        Contribution as PluginContribution, Declared as PluginDeclared, Grant as PluginGrant, Requirement as PluginRequirement,
        Requirements as PluginRequirements,
    };
    pub use crate::{Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError};
    pub use harmos_domain::__private::*;
    pub use harmos_domain::config::SchemaSettings;

    #[cfg(feature = "lifecycle")]
    pub use crate::execution::__private::Catalog;

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    /* === Enums === */

    /* === Traits === */

    /* === Implementations === */

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

/* === Tests === */
