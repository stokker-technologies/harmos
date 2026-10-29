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
use std::sync::{Arc, Mutex};

// Third Party Imports

// Local Imports
use super::{CallContext, Contract, OperationDefinition, OperationGeneration};
use crate::{Error, Identifier, Result};

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

#[derive(Default)]
pub(super) struct Configuration {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    authorizer: Option<Arc<dyn Authorizer>>,
    frozen: bool,
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

impl Configuration {
    pub(super) fn register(&self, authorizer: Arc<dyn Authorizer>) -> Result<()> {
        let mut state = self.state.lock().expect("authorization configuration lock is healthy");
        if state.frozen {
            return Err(Error::Closing {
                domain: "operation authorization configuration",
            });
        }
        if state.authorizer.is_some() {
            return Err(Error::InvalidInput {
                field: "operation authorizer",
                reason: "authorization already has an owner",
            });
        }
        state.authorizer = Some(authorizer);
        Ok(())
    }

    #[cfg(feature = "grpc")]
    pub(super) fn authorizer(&self) -> Option<Arc<dyn Authorizer>> {
        let mut state = self.state.lock().expect("authorization configuration lock is healthy");
        state.frozen = true;
        state.authorizer.clone()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
