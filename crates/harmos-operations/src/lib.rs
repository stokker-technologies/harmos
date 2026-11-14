/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod native;

mod answer;
pub mod catalog;
pub mod config;
pub mod execution;
pub mod failure;
pub mod host;
pub mod inventory;
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

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use native::{
    Error, Result, access, builtins_catalog, builtins_catalog_batch, clock, host_catalog, lifecycle, lifecycle_catalog, plugins, status,
    storage_catalog, streams_catalog,
};

pub use config::{ConfigError, decode_config, encode_config, validate_config};
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

/* === Imports === */

// Standard Library Imports

// Third Party Imports
extern crate self as harmos;
extern crate self as harmos_operations;

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
    pub use crate::plugin::{Grant as PluginGrant, Requirement as PluginRequirement, Requirements as PluginRequirements};
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
