/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub use harmos_domain::inventory::contracts::Required;

use super::{CallContext, Contract, OperationDefinition, OperationGeneration};
use crate::{Identifier, Result};

// Borrow Original Live Provider Metadata; This Is Not Another Catalog.
pub struct Provider<'a> {
    pub owner: &'a Identifier,
    pub plugin: bool,
    pub generation: OperationGeneration,
    pub definition: &'a OperationDefinition,
    pub contract: &'a Contract,
}

#[derive(Clone, Debug)]
pub enum Authorization {
    Full(Contract),
    Restricted(Required),
}

// Synchronous, Bounded And Nonblocking. No Request Probing Or Role Inference.
// None Denies Access; Both Selections Must Match The Original Contract.
pub trait Authorizer: Send + Sync + 'static {
    fn authorize(&self, context: &CallContext, provider: Provider<'_>) -> Result<Option<Authorization>>;
}
