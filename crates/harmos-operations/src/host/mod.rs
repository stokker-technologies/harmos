/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod catalog;

/* === Re-Exports === */

pub use catalog::{Catalog, CatalogKind, CatalogPage};
pub use harmos_domain::model::operations::discovery::DiscoveredOperation;

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

#[harmos::catalog(harmos_path = crate)]
pub enum Host {
    Catalog(Catalog),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
