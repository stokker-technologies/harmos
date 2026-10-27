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

#[cfg(not(feature = "runtime"))]
pub use harmos_domain::*;

#[cfg(any(not(feature = "runtime"), target_arch = "wasm32"))]
pub mod prelude {
    pub use harmos_domain::{
        Backoff, Jitter, Message, OperationDeclaration, OperationDefinition, OperationPolicy, Portable, RestartPolicy, RetryPolicy,
        ServicePolicy, catalog, message, operation,
    };
    #[cfg(feature = "runtime")]
    pub use harmos_sdk::{Context, Dependencies, ExecutionContext, Operation, RegistrationBatch, Response, Service};
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
