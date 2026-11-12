/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use crate::{
    catalog::Node,
    operations::Contract,
    plugin::{Grant, Requirement},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Required {
    pub contract: Contract,

    #[serde(default)]
    pub grants: Vec<Grant>,
}

impl Required {
    pub fn from_tree(entries: &[Node<Requirement>]) -> Result<Vec<Self>, crate::operations::ContractError> {
        let mut required = Vec::with_capacity(crate::catalog::count(entries));
        crate::catalog::try_for_each(entries, |entry| {
            required.push(Self {
                contract: entry.declaration.contract()?,
                grants: entry.grants.to_vec(),
            });
            Ok::<(), crate::operations::ContractError>(())
        })?;
        Ok(required)
    }

    pub fn grants(&self, grant: &Grant) -> bool {
        self.grants.contains(grant)
    }
}
