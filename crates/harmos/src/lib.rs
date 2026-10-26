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

pub use harmos_domain as domain;
#[cfg(feature = "runtime")]
pub use harmos_runtime::*;

#[cfg(not(feature = "runtime"))]
pub use harmos_domain::*;

#[cfg(not(feature = "runtime"))]
pub mod prelude {
    pub use harmos_domain::{
        Backoff, Jitter, Message, OperationDeclaration, OperationDefinition, OperationPolicy, Portable, RequestFor, RestartPolicy,
        RetryPolicy, ServicePolicy, catalog, message, operation,
    };
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
