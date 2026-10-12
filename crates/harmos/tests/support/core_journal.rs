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
use std::sync::atomic::{AtomicUsize, Ordering};
// Third Party Imports

// Local Imports
use crate::support::{Bank, DEPOSIT};
use harmos::operations::{Context, Operation, host};
use harmos::{state::PrincipalId, state::RequestKey, state::StateManager};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct OriginalJournal {
    state: StateManager<Bank>,
    pub prepared: AtomicUsize,
    pub handled: AtomicUsize,
    pub finalized: AtomicUsize,
}

pub struct Invoke {
    resources: Arc<OriginalJournal>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OriginalJournal {
    pub fn new(state: StateManager<Bank>) -> Self {
        Self {
            state,
            prepared: AtomicUsize::new(0),
            handled: AtomicUsize::new(0),
            finalized: AtomicUsize::new(0),
        }
    }
}

impl Operation for Invoke {
    type Declaration = host::Invoke;
    type Resources = OriginalJournal;
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(resources: Arc<OriginalJournal>) -> harmos::Result<Self> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut host::InvokeRequest,
        _endpoints: &mut Self::Endpoints,
    ) -> harmos::Result<host::Receipt> {
        self.resources.handled.fetch_add(1, Ordering::SeqCst);
        let scope = context.caller_scope().ok_or(harmos::Error::InvalidInput {
            field: "journal caller",
            reason: "original generation is required",
        })?;
        if request.grant != DEPOSIT.1 {
            return Err(harmos::Error::OperationUnauthorized {
                caller: scope.owner().into(),
                owner: "bank-journal".into(),
                operation: request.grant.clone(),
            });
        }
        // The Application Owns Its Exact Transaction Version And Original StateManager.
        let journal = self
            .resources
            .state
            .as_principal(PrincipalId::new(format!("guest:{}", scope.owner())));
        let transaction = journal
            .decode_transaction(
                &harmos::DefinitionId::new(DEPOSIT.1),
                DEPOSIT.2,
                &request.payload,
            )
            .map_err(|source| harmos::Error::OperationFailed {
                operation: "host.invoke".into(),
                source: Arc::new(source),
            })?;
        let receipt = journal
            .commit_keyed(
                RequestKey::new(std::mem::take(&mut request.key)),
                transaction,
            )
            .await
            .map_err(|source| harmos::Error::OperationFailed {
                operation: "host.invoke".into(),
                source: Arc::new(source),
            })?;
        Ok(host::Receipt {
            position: receipt.position.0,
            correlation: receipt.correlation.0.0,
        })
    }
    async fn finalize(&mut self) -> harmos::Result<()> {
        self.resources.finalized.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
