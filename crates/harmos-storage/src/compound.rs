/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Admit Compound Atomic Mutations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::io;
use std::sync::{Arc, OnceLock};

// Third Party Imports
use serde::Serialize;
use sha2::{Digest, Sha256};

// Local Imports
use crate::admissions::{Fingerprint, Mutation};
use crate::capacity::{Attempt, Refusal};
use crate::manager::{Domain, Operation, State};
use crate::receipts::ReceiptState;
use crate::schema::graph::Schema;
use crate::{
    BackendFailure, CompoundOutcome, CompoundReceipt, Error, Identifier, MemorySize, MutationHeader, MutationId, MutationItem,
    MutationReceipt, MutationValue, ObjectEntry, ObjectKey, ObjectVersionToken, Result, StorageCursor, StorageManager, StorageSchema,
    StoredRecord,
};
use harmos_domain::config::SchemaSettings;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// A single backend transaction. Persist its receipt with its effects before returning.
pub struct CompoundExchange {
    pub position: StorageCursor,
    pub items: Vec<CompoundWrite>,
    pub(super) header: OnceLock<MutationHeader>,
}

/* === Enums === */

#[derive(Clone, Debug, Serialize)]
pub enum CompoundWrite {
    Object { expected: Option<ObjectVersionToken>, schema: StorageSchema, value: ObjectEntry },
    Delete { position: StorageCursor, schema: Identifier, item: ObjectKey, expected: Option<ObjectVersionToken> },
    Record(StoredRecord),
    Scope { position: StorageCursor, scope: Identifier },
}

/* === Traits === */

/* === Implementations === */

impl CompoundWrite {
    pub fn position(&self) -> &StorageCursor {
        match self {
            Self::Object { value, .. } => &value.position,
            Self::Delete { position, .. } => position,
            Self::Record(record) => &record.position,
            Self::Scope { position, .. } => position,
        }
    }
}

impl CompoundExchange {
    pub fn mutation(&self) -> &MutationHeader {
        self.header.get().expect("published compound mutation")
    }

    pub fn receipt(&self, outcome: CompoundOutcome) -> MutationReceipt {
        self.mutation().complete(MutationValue::Compound(CompoundReceipt { position: self.position.clone(), outcome }))
    }

    pub fn replay(&self, receipt: &MutationReceipt) -> Result<CompoundReceipt> {
        if receipt.mutation != self.mutation().mutation || receipt.fingerprint != self.mutation().fingerprint {
            return Err(Error::backend(
                "commit compound mutation",
                BackendFailure::Conflict,
                io::Error::other("mutation identity has different items; use a new mutation identity"),
            ));
        }
        match &receipt.value {
            MutationValue::Compound(value) if value.position == self.position => {
                match &value.outcome {
                    CompoundOutcome::Applied { positions } if positions.iter().eq(self.items.iter().map(CompoundWrite::position)) => {}
                    CompoundOutcome::Conflict { index, current } if *index < self.items.len() => {
                        if matches!(self.items[*index], CompoundWrite::Record(_) | CompoundWrite::Scope { .. }) {
                            return Err(invalid("backend returned a condition for a record append; repair the backend"));
                        }
                        if let Some(current) = current {
                            self.position.history.check(current.history())?;
                        }
                    }
                    _ => return Err(invalid("backend returned an invalid compound outcome; repair the backend")),
                }
                Ok(value.clone())
            }
            _ => Err(invalid("mutation owns a different result; use a new mutation identity")),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.items.is_empty() {
            return Err(invalid("supply at least one mutation item"));
        }
        let mut previous = None;
        for item in &self.items {
            let position = item.position();
            self.position.history.check(&position.history)?;
            let first = match item {
                CompoundWrite::Object { expected, schema, value } => {
                    value.version.check_position(position)?;
                    self.position.history.check(&schema.position.history)?;
                    if schema.key != value.value.schema || schema.position.sequence.checked_add(1) != Some(position.sequence) {
                        return Err(invalid("supply consecutive matching schema and object positions"));
                    }
                    if let Some(expected) = expected {
                        self.position.history.check(expected.history())?;
                    }
                    schema.encoded_bytes()?;
                    schema.position.sequence
                }
                CompoundWrite::Delete { expected, .. } => {
                    if let Some(expected) = expected {
                        self.position.history.check(expected.history())?;
                    }
                    position.sequence
                }
                CompoundWrite::Record(_) => position.sequence,
                CompoundWrite::Scope { .. } => {
                    if self.items.len() != 1 {
                        return Err(invalid("delete one scope in a separate mutation"));
                    }
                    position.sequence
                }
            };
            if first == 0 || previous.is_some_and(|previous: u64| previous.checked_add(1) != Some(first)) {
                return Err(invalid("supply a consecutive compound position range"));
            }
            previous = Some(position.sequence);
        }
        if previous != Some(self.position.sequence) {
            return Err(invalid("supply the last item position as the compound frontier"));
        }
        Ok(())
    }
}

impl StorageManager {
    #[tracing::instrument(name = "storage.compound.mutate_once", level = "debug", skip_all)]
    pub async fn mutate_once(&self, mutation: &MutationId, items: Vec<MutationItem>) -> Result<CompoundReceipt> {
        if !self.capabilities().compound_mutations {
            return Err(Error::backend(
                "commit compound mutation",
                BackendFailure::Unsupported,
                io::Error::other("register a backend implementing atomic compound mutations"),
            ));
        }
        if items.is_empty() {
            return Err(invalid("supply at least one mutation item"));
        }
        if items.len() != 1 && items.iter().any(|item| matches!(item, MutationItem::DeleteScope(_))) {
            return Err(invalid("delete one scope in a separate mutation"));
        }
        if items.memory_size() > self.inner.settings.max_batch_bytes().get() {
            return Err(Error::ResourceExhausted {
                resource: "compound mutation; reduce the item batch",
            });
        }
        let mut hash = Fingerprint(Sha256::new());
        hash.0.update(b"storage.compound:v1\0");
        serde_json::to_writer(&mut hash, &items)
            .map_err(|source| Error::backend("normalize compound items", BackendFailure::Permanent, source))?;
        let fingerprint = hash.0.finalize().into();
        let count = items.iter().try_fold(0usize, |count, item| {
            let positions = match item {
                MutationItem::Create(_) | MutationItem::Set { .. } => 2,
                _ => 1,
            };
            count.checked_add(positions).ok_or_else(|| invalid("reduce the compound item count"))
        })?;
        let _writer = self.inner.object_writes.lock().await;
        let (sequence, completion) = self
            .admit_mutation_owned(
                Domain::Data,
                self.inner.settings.max_batch_bytes().get(),
                count,
                Some(Mutation { id: mutation, fingerprint }),
                false,
                |first| {
                    let mut writes = Vec::with_capacity(items.len());
                    let mut sequence = first;
                    let item_count = items.len();
                    for (index, item) in items.into_iter().enumerate() {
                        let expected = match &item {
                            MutationItem::Set { expected, .. } => Some(expected.clone()),
                            _ => None,
                        };
                        let write = match item {
                            MutationItem::Create(value) | MutationItem::Set { value, .. } => {
                                self.inner.catalog.validate_value(&value, self.inner.settings.max_batch_bytes())?;
                                let definition = self.inner.catalog.definition(&value.schema)?;
                                let graph: Schema = definition.graph(&SchemaSettings::default())?;
                                let schema = StorageSchema::new(self.position(sequence), value.schema.clone(), graph);
                                sequence += 1;
                                let position = self.position(sequence);
                                CompoundWrite::Object {
                                    expected,
                                    schema,
                                    value: ObjectEntry {
                                        version: ObjectVersionToken::new(position.clone()),
                                        position,
                                        value,
                                    },
                                }
                            }
                            MutationItem::Delete { schema, item, expected } => CompoundWrite::Delete {
                                position: self.position(sequence),
                                schema,
                                item,
                                expected,
                            },
                            MutationItem::Append(record) => CompoundWrite::Record(record.stored(self.position(sequence))),
                            MutationItem::DeleteScope(scope) => CompoundWrite::Scope { position: self.position(sequence), scope },
                        };
                        if index + 1 < item_count {
                            sequence += 1;
                        }
                        writes.push(write);
                    }
                    Ok(Operation::Compound(Arc::new(CompoundExchange {
                        position: self.position(sequence),
                        items: writes,
                        header: OnceLock::new(),
                    })))
                },
            )
            .await?;
        let completion = completion.expect("compound admission owns completion");
        self.flush_until(&self.position(sequence)).await?;
        self.inner
            .wait(None, |state| match state.receipts.lookup(mutation, &fingerprint, Some(&completion)) {
                Ok(ReceiptState::Complete(MutationValue::Compound(receipt))) => Attempt::Ready(receipt),
                Ok(ReceiptState::Pending) => match &state.status.failure {
                    Some(source) => Attempt::Refuse(Refusal::Failed(Error::Persistence { source: source.clone() })),
                    None => Attempt::Wait,
                },
                Ok(_) => Attempt::Refuse(Refusal::Failed(invalid("mutation owns a different result; use a new identity"))),
                Err(error) => Attempt::Refuse(Refusal::Failed(error)),
            })
            .await
            .map_err(Refusal::into_error)
    }
}

impl State {
    pub(super) fn deleting_scope(&self, scope: &Identifier) -> bool {
        self.queue.iter().any(|pending| match &pending.operation {
            Operation::Compound(exchange) => {
                exchange.items.iter().any(|item| matches!(item, CompoundWrite::Scope { scope: selected, .. } if selected == scope))
            }
            _ => false,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn invalid(reason: &'static str) -> Error {
    Error::InvalidInput { field: "compound mutation", reason }
}

/* === Tests === */
