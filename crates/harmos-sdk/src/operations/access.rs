/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::sync::Arc;

pub type AccessError = Box<dyn std::error::Error + Send + Sync>;
pub type DiscoveryPage = (Vec<u8>, Option<String>);

#[derive(Clone)]
pub struct DiscoveryAccess(pub Arc<dyn Discovery>);
#[derive(Clone)]
pub struct GrantAccess(pub Arc<dyn GrantAuthority>);

pub trait Discovery: Send + Sync {
    fn discover(&self, after: Option<&str>, rows: usize, bytes: usize) -> Result<DiscoveryPage, AccessError>;
}
pub trait GrantAuthority: Send + Sync {
    fn admit(&self, grant: &harmos_domain::plugin::Grant) -> Result<(), AccessError>;
}
