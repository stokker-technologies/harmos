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
mod catalog;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod registration;

/* === Re-Exports === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use catalog::{Catalog, limits};
pub use harmos_protocol::host::DiscoveredOperation;
pub use harmos_protocol::host::{CatalogKind, CatalogPage};
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use registration::Host;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = harmos_domain::OperationPolicy {
    timeout: std::time::Duration::from_secs(30),
    attempt_timeout: std::time::Duration::from_secs(30),
    cancellation_timeout: std::time::Duration::ZERO,
    retries: harmos_domain::RetryPolicy::Never,
};

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
