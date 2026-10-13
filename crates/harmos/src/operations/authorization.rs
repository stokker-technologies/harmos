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
pub use harmos_contract::inventory::contracts::Required;

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{CallContext, Contract, Definition, OperationGeneration};
use crate::{Identifier, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// Borrow Original Live Provider Metadata; This Is Not Another Catalog.
pub struct Provider<'a> {
    pub owner: &'a Identifier,
    pub generation: OperationGeneration,
    pub definition: &'a Definition,
    pub contract: &'a Contract,
}

/* === Enums === */

/* === Traits === */
// Synchronous, Bounded And Nonblocking. No Request Probing Or Role Inference.
// None Denies Access; A Selected Requirement Must Match The Original Contract.
pub trait Authorizer: Send + Sync + 'static {
    fn required(&self, context: &CallContext, provider: Provider<'_>) -> Result<Option<Required>>;
}

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
