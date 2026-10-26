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

pub use harmos_domain::inventory::contracts::Required;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{CallContext, Contract, OperationDefinition, OperationGeneration};
use crate::{Identifier, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// Borrow Original Live Provider Metadata; This Is Not Another Catalog.
pub struct Provider<'a> {
    pub owner: &'a Identifier,
    pub plugin: bool,
    pub generation: OperationGeneration,
    pub definition: &'a OperationDefinition,
    pub contract: &'a Contract,
}

/* === Enums === */

#[derive(Clone, Debug)]
pub enum Authorization {
    Full(Contract),
    Restricted(Required),
}

/* === Traits === */

// Synchronous, Bounded And Nonblocking. No Request Probing Or Role Inference.
// None Denies Access; Both Selections Must Match The Original Contract.
pub trait Authorizer: Send + Sync + 'static {
    fn authorize(&self, context: &CallContext, provider: Provider<'_>) -> Result<Option<Authorization>>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
