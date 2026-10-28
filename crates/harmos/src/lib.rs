/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Portable Contracts Are Available Without The Execution Runtime.

/* === Modules === */

/* === Re-Exports === */

pub use harmos_operations as builtins;

pub use harmos_domain as domain;
#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use harmos_runtime::*;

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
pub use harmos_sdk::*;

// Portable operation declarations remain portable even when the same dependency graph
// enables runtime execution for another crate.
pub use harmos_macros::{catalog, operation};

#[cfg(all(feature = "sidecar", not(target_arch = "wasm32")))]
pub mod sidecar {
    pub use harmos_runtime::sidecar::*;
}
#[cfg(all(feature = "sidecar", not(target_arch = "wasm32")))]
pub use harmos_macros::plugin as sidecar;

#[cfg(all(feature = "guest", not(target_arch = "wasm32")))]
pub mod guest {
    pub use harmos_runtime::guest::*;
}
#[cfg(all(feature = "guest", target_arch = "wasm32"))]
pub mod guest {
    pub use harmos_guest::*;
    pub use harmos_macros::plugin;
}
#[cfg(feature = "guest")]
pub use harmos_macros::plugin as guest;

#[cfg(all(feature = "guest", target_arch = "wasm32"))]
#[doc(hidden)]
pub mod __private {
    pub use harmos_domain::__private::{CarrierContext, owner_segment};
    pub use harmos_domain::config::SchemaSettings;
    pub use harmos_domain::inventory as PluginInventory;
    pub use harmos_domain::plugin::schema_markers as PluginSchemaMarkers;
    pub use harmos_domain::plugin::{Contribution as PluginContribution, Grant as PluginGrant};
    pub use harmos_domain::plugin::{Declared as PluginDeclared, Requirement as PluginRequirement, Requirements as PluginRequirements};
    pub use harmos_domain::{Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError};
    pub use harmos_sdk::__private::*;
}

#[cfg(not(feature = "runtime"))]
pub use harmos_domain::*;

#[cfg(any(not(feature = "runtime"), target_arch = "wasm32"))]
pub mod prelude {
    pub use crate::{catalog, operation};
    pub use harmos_domain::{
        Backoff, Jitter, Message, OperationDeclaration, OperationDefinition, OperationPolicy, Portable, RestartPolicy, RetryPolicy,
        ServicePolicy, message,
    };
    #[cfg(feature = "runtime")]
    pub use harmos_sdk::{Context, ExecutionContext, Operation, Response, Service};
}

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
