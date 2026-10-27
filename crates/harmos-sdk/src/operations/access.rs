/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use std::sync::Arc;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type AccessError = Box<dyn std::error::Error + Send + Sync>;
pub type DiscoveryPage = (Vec<u8>, Option<String>);

/* === Structs === */

#[derive(Clone)]
pub struct DiscoveryAccess(pub Arc<dyn Discovery>);
#[derive(Clone)]
pub struct GrantAccess(pub Arc<dyn GrantAuthority>);

/* === Enums === */

/* === Traits === */

pub trait Discovery: Send + Sync {
    fn discover(&self, after: Option<&str>, rows: usize, bytes: usize) -> Result<DiscoveryPage, AccessError>;
}
pub trait GrantAuthority: Send + Sync {
    fn admit(&self, grant: &harmos_domain::plugin::Grant) -> Result<(), AccessError>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
