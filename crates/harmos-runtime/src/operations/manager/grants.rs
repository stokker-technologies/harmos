/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */

impl GrantAuthority {
    // Refuse a request whose grant the caller does not hold. A request resolves
    // only to a grant on the operation it calls; a provider preparing a grant on
    // another operation is refused for every caller.
    pub(in crate::operations) fn admit(&self, grant: Option<&harmos_domain::plugin::Grant>) -> Result<()> {
        let denied = |caller: &str| Error::OperationUnauthorized {
            caller: caller.to_owned(),
            owner: self.owner.clone(),
            operation: self.operation.clone(),
        };
        if grant.is_some_and(|grant| grant.operation() != self.operation) {
            return Err(Error::PermissionDenied);
        }
        #[cfg(any(
            feature = "plugin",
            feature = "plugin-guest",
            feature = "plugin-sidecar"
        ))]
        if let Some((caller, required)) = &self.plugin
            && !grant_allowed(required.as_ref(), grant)
        {
            return Err(denied(caller));
        }
        if let Some(required) = &self.external
            && grant.is_some_and(|grant| !required.grants(grant))
        {
            return Err(denied("external"));
        }
        Ok(())
    }
}

