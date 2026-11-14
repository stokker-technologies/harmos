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

use harmos_protocol::model::DiscoveredOperation;

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

/// Trusted Plugin Identity Supplied By The Runtime Carrier.
#[derive(Clone)]
pub struct CallingPlugin {
    pub name: Option<String>,
}

/* === Enums === */

/* === Traits === */

pub trait Discovery: Send + Sync {
    fn discover(&self) -> Result<Vec<DiscoveredOperation>, AccessError>;
}
pub trait GrantAuthority: Send + Sync {
    fn admit(&self, grant: &harmos_operations::plugin::Grant) -> Result<(), AccessError>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
