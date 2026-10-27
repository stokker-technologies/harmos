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

pub use catalog::{Catalog, CatalogKind, CatalogPage, CatalogRequest};
pub use harmos_domain::model::operations::discovery::DiscoveredOperation;

/* === Constants === */

pub const POLICY: harmos_domain::OperationPolicy = crate::lifecycle::POLICY;

/* === Enums === */

#[harmos::catalog(harmos_path = crate)]
pub enum Host {
    Catalog(Catalog),
}
