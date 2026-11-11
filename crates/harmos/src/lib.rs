/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Host Execution and Plugin Placement Are Available by Default.

/* === Modules === */

/* === Re-Exports === */

pub use harmos_domain as domain;
pub use harmos_operations as builtins;

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use harmos_runtime::*;

#[cfg(all(feature = "runtime", not(target_arch = "wasm32")))]
pub use harmos_sdk_grpc as sdk;

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
pub use harmos_operations::execution::*;

#[cfg(all(feature = "guest", target_arch = "wasm32"))]
pub use harmos_guest::{Plugin, plugin};

// Portable operation declarations remain portable even when the same dependency graph
// enables runtime execution for another crate.
pub use harmos_macros::{catalog, operation};

#[cfg(feature = "guest")]
pub use harmos_guest::guest;

#[cfg(not(feature = "runtime"))]
pub use harmos_domain::*;

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

#[cfg(all(feature = "guest", not(target_arch = "wasm32")))]
pub mod guest {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_runtime::guest::*;

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

#[cfg(all(feature = "guest", target_arch = "wasm32"))]
pub mod guest {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_guest::guest as plugin;
    pub use harmos_guest::*;

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

#[cfg(all(feature = "guest", target_arch = "wasm32"))]
#[doc(hidden)]
pub mod __private {
    /* === Modules === */

    /* === Re-Exports === */

    pub use harmos_domain::__private::{CarrierContext, owner_segment};
    pub use harmos_domain::config::SchemaSettings;
    pub use harmos_domain::inventory as PluginInventory;
    pub use harmos_domain::plugin::schema_markers as PluginSchemaMarkers;
    pub use harmos_domain::plugin::{Contribution as PluginContribution, Grant as PluginGrant};
    pub use harmos_domain::plugin::{Declared as PluginDeclared, Requirement as PluginRequirement, Requirements as PluginRequirements};
    pub use harmos_domain::{Registrar as PluginRegistrar, RegistrationError as PluginRegistrationError};
    pub use harmos_operations::execution::__private::*;

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

#[cfg(any(not(feature = "runtime"), target_arch = "wasm32"))]
pub mod prelude {
    /* === Modules === */

    /* === Re-Exports === */

    pub use crate::{catalog, operation};
    pub use harmos_domain::{
        Backoff, Jitter, Message, OperationDeclaration, OperationDefinition, OperationPolicy, Portable, RestartPolicy, RetryPolicy,
        ServicePolicy, message,
    };

    #[cfg(feature = "runtime")]
    pub use harmos_operations::execution::{Context, ExecutionContext, Invocation, Operation, OperationHost, Response, Service};

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
