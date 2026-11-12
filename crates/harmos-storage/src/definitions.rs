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

use crate::manager::{Inner, State, Target};
use crate::query::{columnar::Cancellation, index::IndexDefinition, projection::ProjectionDefinition};
use crate::receipts::ReceiptState;
use crate::{
    BackendFailure, Error, Identifier, MutationHeader, MutationId, MutationReceipt, MutationValue, ReadLimits, Result, StorageManager,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/* === Structs === */

pub(super) struct DefinitionPending {
    work: Arc<DefinitionWork>,
    failure: Option<Arc<Error>>,
    retrying: bool,
}
pub(super) struct DefinitionWork {
    pub(super) context: crate::execution::Context,
    pub(super) queued: std::sync::Mutex<Option<tracing::Span>>,
    pub(super) header: MutationHeader,
    input: DefinitionInput,
    target: Target,
    bytes: usize,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionKind {
    Index,
    Projection,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum DefinitionOutcome {
    Applied { revision: u64 },
    Refused { kind: DefinitionRefusal, reason: String },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionRefusal {
    Conflict,
    Unsupported,
    Invalid,
    Exhausted,
    Unavailable,
}
#[derive(Clone)]
pub enum DefinitionInput {
    Index { value: IndexDefinition, expected: Option<u64>, rebuild: bool, work: ReadLimits },
    Projection { value: ProjectionDefinition, expected: Option<u64>, rebuild: bool },
}

/* === Implementations === */

impl DefinitionKind {
    pub fn action(self) -> &'static str {
        match self {
            Self::Index => "SetIndexDefinition",
            Self::Projection => "SetProjectionDefinition",
        }
    }
}
impl DefinitionOutcome {
    pub fn capture(result: Result<u64>) -> Result<Self> {
        match result {
            Ok(revision) if revision > 0 => Ok(Self::Applied { revision }),
            Ok(_) => Err(invalid("provider returned zero definition revision")),
            Err(error) => {
                let kind = match &error {
                    Error::Backend { kind: BackendFailure::Conflict, .. } => DefinitionRefusal::Conflict,
                    Error::Backend { kind: BackendFailure::Unsupported, .. } => DefinitionRefusal::Unsupported,
                    Error::InvalidInput { .. } | Error::Domain(_) | Error::IndexNotFound { .. } => DefinitionRefusal::Invalid,
                    Error::ResourceExhausted { .. } => DefinitionRefusal::Exhausted,
                    Error::NotReady { .. } | Error::Gap { .. } => DefinitionRefusal::Unavailable,
                    _ => return Err(error),
                };
                Ok(Self::Refused { kind, reason: error.to_string() })
            }
        }
    }
    pub(super) fn validate(&self) -> Result<()> {
        if matches!(self, Self::Applied { revision: 0 }) {
            return Err(invalid("provider returned zero definition revision"));
        }
        Ok(())
    }
    pub fn into_result(self, owner: DefinitionKind) -> Result<u64> {
        match self {
            Self::Applied { revision } => Ok(revision),
            Self::Refused { kind: DefinitionRefusal::Exhausted, .. } => Err(Error::ResourceExhausted { resource: owner.action() }),
            Self::Refused { kind: DefinitionRefusal::Unavailable, .. } => Err(Error::NotReady { domain: owner.action() }),
            Self::Refused { kind, reason } => Err(Error::backend(
                owner.action(),
                match kind {
                    DefinitionRefusal::Conflict => BackendFailure::Conflict,
                    DefinitionRefusal::Unsupported => BackendFailure::Unsupported,
                    _ => BackendFailure::Permanent,
                },
                std::io::Error::other(reason),
            )),
        }
    }
    pub(super) fn heap_bytes(&self) -> usize {
        match self {
            Self::Applied { .. } => 0,
            Self::Refused { reason, .. } => reason.capacity(),
        }
    }
}
impl DefinitionInput {
    fn kind(&self) -> DefinitionKind {
        match self {
            Self::Index { .. } => DefinitionKind::Index,
            Self::Projection { .. } => DefinitionKind::Projection,
        }
    }
    fn name(&self) -> &Identifier {
        match self {
            Self::Index { value, .. } => &value.name,
            Self::Projection { value, .. } => &value.name,
        }
    }
    pub fn fingerprint(&self) -> Result<[u8; 32]> {
        let mut hash = Sha256::new();
        hash.update(b"storage.definition:v1\0");
        hash.update(self.encoded()?);
        Ok(hash.finalize().into())
    }
    fn encoded(&self) -> Result<Vec<u8>> {
        match self {
            Self::Index { value, expected, rebuild, work } => {
                serde_json::to_vec(&(self.kind(), value, expected, rebuild, work.rows.get(), work.bytes.get()))
            }
            Self::Projection { value, expected, rebuild } => serde_json::to_vec(&(self.kind(), value, expected, rebuild)),
        }
        .map_err(|source| Error::backend("encode definition mutation", BackendFailure::Permanent, source))
    }
}
impl StorageManager {
    /// Test Admission Before Looking Up Mutable Definition State During A Retry.
    pub fn definition_admitted(&self, mutation: &MutationId, input: &DefinitionInput) -> Result<bool> {
        let state = self.inner.state.lock().unwrap();
        crate::capacity::admitting(&state, false)?;
        Ok(!matches!(state.receipts.lookup(mutation, &input.fingerprint()?, None)?, ReceiptState::Missing))
    }
    pub async fn set_index_definition_once(
        &self, mutation: &MutationId, value: IndexDefinition, expected: Option<u64>, rebuild: bool, work: ReadLimits,
    ) -> Result<u64> {
        value.validate()?;
        self.definition_once(mutation, DefinitionInput::Index { value, expected, rebuild, work }).await
    }
    pub async fn set_projection_definition_once(
        &self, mutation: &MutationId, value: ProjectionDefinition, expected: Option<u64>, rebuild: bool,
    ) -> Result<u64> {
        value.validate()?;
        self.definition_once(mutation, DefinitionInput::Projection { value, expected, rebuild }).await
    }
    async fn definition_once(&self, mutation: &MutationId, input: DefinitionInput) -> Result<u64> {
        if !self.inner.primary.capabilities().mutation_receipts {
            return Err(Error::backend(
                input.kind().action(),
                BackendFailure::Unsupported,
                std::io::Error::other("provider must retain mutation receipts"),
            ));
        }
        let encoded = input.encoded()?;
        let fingerprint = input.fingerprint()?;
        let bytes = encoded.len().saturating_add(std::mem::size_of::<DefinitionWork>()).saturating_add(2 * input.name().as_str().len());
        if bytes > self.inner.settings.request.max_pending_write_bytes.get() {
            return Err(exhausted());
        }
        let mut changed = self.inner.changed.subscribe();
        let mut completion = None;
        let mut retry = true;
        loop {
            let mut notify = false;
            {
                let mut state = self.inner.state.lock().unwrap();
                crate::capacity::admitting(&state, false)?;
                match state.receipts.lookup(mutation, &fingerprint, completion.as_ref())? {
                    ReceiptState::Complete(MutationValue::Definition { kind, name, outcome })
                        if kind == input.kind() && &name == input.name() =>
                    {
                        return outcome.into_result(kind);
                    }
                    ReceiptState::Complete(_) => return Err(invalid("mutation identity owns a different result")),
                    ReceiptState::Pending => {
                        if completion.is_none() {
                            completion = Some(state.receipts.completion(mutation));
                        }
                        let pending =
                            state.definition_mutations.get(mutation).ok_or_else(|| invalid("pending identity owns different work"))?;
                        if pending.retrying { /* The Callee Still Owns Its Retry. */
                        } else if let Some(source) = pending.failure.clone() {
                            if !retry {
                                return Err(Error::Persistence { source });
                            }
                            if let Some(other) = &state.status.failure
                                && !Arc::ptr_eq(other, &source)
                            {
                                return Err(Error::Persistence { source: other.clone() });
                            }
                            state.definition_mutations.get_mut(mutation).unwrap().failure = None;
                            state.definition_mutations.get_mut(mutation).unwrap().retrying = false;
                            state.status.failure = None;
                            state.retry = state.retry.wrapping_add(1);
                            notify = true;
                        } else if let Some(source) = &state.status.failure {
                            return Err(Error::Persistence { source: source.clone() });
                        }
                        retry = false;
                    }
                    ReceiptState::Missing => {
                        if let Some(source) = &state.status.failure {
                            return Err(Error::Persistence { source: source.clone() });
                        }
                        state.collect();
                        if state.status.pending_bytes.saturating_add(bytes) <= self.inner.settings.request.max_pending_write_bytes.get()
                            && state.status.pending_batches < self.inner.settings.request.max_pending_write_batches.get()
                        {
                            state.receipts.insert(mutation, fingerprint)?;
                            completion = Some(state.receipts.completion(mutation));
                            let header = state.receipts.header(mutation);
                            let target = state.data_accepted;
                            state.definition_mutations.insert(
                                mutation.clone(),
                                DefinitionPending {
                                    work: Arc::new(DefinitionWork {
                                        context: crate::execution::Context::current(),
                                        queued: std::sync::Mutex::new(Some(tracing::debug_span!("storage.definition.queue"))),
                                        header,
                                        input: input.clone(),
                                        target,
                                        bytes,
                                    }),
                                    failure: None,
                                    retrying: false,
                                },
                            );
                            retry = false;
                            state.reserved += bytes;
                            state.reserved_batches += 1;
                            state.collect();
                            notify = true;
                        } else {
                            self.inner.relieve_pressure(&mut state, bytes, 1);
                        }
                    }
                }
            }
            if notify {
                self.inner.notify();
            }
            changed.changed().await.map_err(|_| Error::Closing { domain: "storage definition mutation" })?;
        }
    }
    pub(super) async fn settle_definitions(&self) -> Result<()> {
        let mut changed = self.inner.changed.subscribe();
        {
            let mut state = self.inner.state.lock().unwrap();
            if state.definition_mutations.is_empty() {
                return Ok(());
            }
            for pending in state.definition_mutations.values_mut() {
                pending.failure = None;
                pending.retrying = false;
            }
            state.status.failure = None;
            state.retry = state.retry.wrapping_add(1);
        }
        self.inner.notify();
        loop {
            {
                let state = self.inner.state.lock().unwrap();
                if state.definition_mutations.is_empty() {
                    return Ok(());
                }
                if let Some(source) = &state.status.failure {
                    return Err(Error::Persistence { source: source.clone() });
                }
                if !state.status.running {
                    return Err(Error::NotReady { domain: "definition worker" });
                }
            }
            changed.changed().await.map_err(|_| Error::Closing { domain: "storage definition mutation" })?;
        }
    }
}
impl State {
    pub(super) fn request_definitions(&mut self) {
        for pending in self.definition_mutations.values().filter(|pending| pending.failure.is_none()) {
            let target = pending.work.target;
            if target.index > self.data_flushed && self.data_request.is_none_or(|current| target.index < current.index) {
                self.data_request = Some(target);
            }
        }
    }
    pub(super) fn definition_action(&self) -> Option<Arc<DefinitionWork>> {
        self.definition_mutations
            .values()
            .find(|pending| pending.failure.is_none() && pending.work.target.index <= self.data_flushed)
            .map(|pending| pending.work.clone())
    }
    pub(super) fn definition_completed(&mut self, work: &DefinitionWork, receipt: MutationReceipt) -> Result<()> {
        if receipt.mutation != work.header.mutation
            || receipt.fingerprint != work.header.fingerprint
            || receipt.expires_at != work.header.expires_at
        {
            return Err(invalid("provider changed the definition receipt"));
        }
        if !matches!(&receipt.value, MutationValue::Definition {kind,name,..} if *kind == work.input.kind() && name == work.input.name()) {
            return Err(invalid("provider changed the definition result"));
        }
        self.receipts.complete(&receipt.mutation, receipt.value);
        self.definition_mutations.remove(&receipt.mutation);
        self.reserved -= work.bytes;
        self.reserved_batches -= 1;
        Ok(())
    }
    pub(super) fn definition_failed(&mut self, mutation: &MutationId, source: Arc<Error>, retrying: bool) {
        if let Some(pending) = self.definition_mutations.get_mut(mutation) {
            pending.failure = Some(source);
            pending.retrying = retrying;
        }
    }
    pub(super) fn definition_data_failed(&mut self, index: u64, source: Arc<Error>) {
        for pending in self.definition_mutations.values_mut().filter(|pending| pending.work.target.index >= index) {
            pending.failure = Some(source.clone());
        }
    }
}
impl DefinitionWork {
    pub(super) async fn perform(&self, inner: &Arc<Inner>) -> Result<MutationReceipt> {
        let result = match &self.input {
            DefinitionInput::Index { value, expected, rebuild, work } => {
                inner
                    .bounded(
                        "primary.set_index_definition",
                        inner.primary.set_index_definition(value, *expected, *rebuild, *work, &Cancellation::default(), Some(&self.header)),
                    )
                    .await
            }
            DefinitionInput::Projection { value, expected, rebuild } => {
                inner
                    .bounded(
                        "primary.set_projection_definition",
                        inner.primary.set_projection_definition(value, *expected, *rebuild, Some(&self.header)),
                    )
                    .await
            }
        };
        let unsupported = result.as_ref().is_err_and(|error| error.backend_failure() == Some(BackendFailure::Unsupported));
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) if unsupported => DefinitionOutcome::capture(Err(error))?,
            Err(error) => return Err(error),
        };
        outcome.validate()?;
        let receipt = self.header.complete(MutationValue::Definition {
            kind: self.input.kind(),
            name: self.input.name().clone(),
            outcome,
        });
        if unsupported {
            inner
                .bounded(
                    "primary.append_mutation_receipts",
                    inner.primary.append_mutation_receipts(crate::StorageBatch::new(vec![receipt.clone()])),
                )
                .await?;
        }
        Ok(receipt)
    }
}

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput {
        field: "storage definition mutation",
        reason,
    }
}
fn exhausted() -> Error {
    Error::ResourceExhausted { resource: "storage definition mutations" }
}
