/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod catalog;

pub use catalog::{Catalog, CatalogKind, CatalogPage};
pub use harmos_domain::model::operations::discovery::DiscoveredOperation;

pub const POLICY: harmos_domain::OperationPolicy = crate::lifecycle::POLICY;

#[harmos::catalog(harmos_path = crate)]
pub enum Host {
    Catalog(Catalog),
}
