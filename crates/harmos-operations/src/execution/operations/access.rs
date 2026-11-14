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

// Standard Library Imports
use std::sync::Arc;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type AccessError = Box<dyn std::error::Error + Send + Sync>;

/* === Structs === */

#[derive(Clone)]
pub struct DiscoveryAccess(pub Arc<dyn Discovery>);
#[derive(Clone)]
pub struct GrantAccess(pub Arc<dyn GrantAuthority>);

/* === Enums === */

/* === Traits === */

pub trait Discovery: Send + Sync {
    fn discover(&self) -> Result<Vec<harmos_protocol::v1::DiscoveredOperation>, AccessError>;
}
pub trait GrantAuthority: Send + Sync {
    fn admit(&self, grant: &harmos_operations::plugin::Grant) -> Result<(), AccessError>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
