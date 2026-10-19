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
        #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
        if let Some((caller, required)) = &self.plugin
            && !grant_allowed(required.as_ref(), grant)
        {
            return Err(denied(caller));
        }
        if let Some(crate::operations::authorization::Authorization::Restricted(required)) = &self.external
            && grant.is_some_and(|grant| !required.grants(grant))
        {
            return Err(denied("external"));
        }
        Ok(())
    }
}

/* === Tests === */
#[cfg(all(test, any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
mod tests {
    use super::*;
    use crate::operations::authorization::{Authorization, Required};
    use crate::operations::{WireDeclaration, host::Catalog};
    use harmos_domain::plugin::Grant;

    #[test]
    fn external_full_control_preserves_plugin_grants() {
        let contract = WireDeclaration::of::<Catalog>().contract().unwrap();
        let allowed = Grant::of("host.catalog", "allowed");
        let refused = Grant::of("host.catalog", "refused");
        let foreign = Grant::of("other.operation", "allowed");
        let mut authority = GrantAuthority {
            owner: "host".into(),
            operation: "host.catalog".into(),
            plugin: Some(("caller".into(), Some(Required { contract: contract.clone(), grants: vec![allowed.clone()] }))),
            external: Some(Authorization::Full(contract)),
        };

        authority.admit(Some(&allowed)).unwrap();
        assert!(matches!(authority.admit(Some(&refused)), Err(Error::OperationUnauthorized { caller, .. }) if caller == "caller"));
        assert!(matches!(authority.admit(Some(&foreign)), Err(Error::PermissionDenied)));

        authority.plugin = None;
        authority.admit(Some(&refused)).unwrap();
        assert!(matches!(authority.admit(Some(&foreign)), Err(Error::PermissionDenied)));
    }
}
