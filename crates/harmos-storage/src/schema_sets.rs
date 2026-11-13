/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::receipts::ReceiptState;
use crate::{
    DefinitionKind, DefinitionOutcome, Error, MutationHeader, MutationId, MutationReceipt, MutationValue, Result, SchemaKey, StorageBatch,
    StorageCursor, StorageManager, StorageSchema,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};

/* === Structs === */

pub struct SchemaExchange {
    pub value: StorageSchema,
    pub expected: Option<StorageCursor>,
    pub(super) header: OnceLock<MutationHeader>,
}

pub struct SchemaWriteReceipt {
    pub saved: StorageCursor,
    pub mutation: Option<MutationReceipt>,
}

/* === Enums === */

pub enum SchemaWrite {
    Append(StorageBatch<StorageSchema>),
    Set(Arc<SchemaExchange>),
}

/* === Implementations === */

impl From<StorageBatch<StorageSchema>> for SchemaWrite {
    fn from(value: StorageBatch<StorageSchema>) -> Self {
        Self::Append(value)
    }
}
impl SchemaExchange {
    pub fn mutation(&self) -> &MutationHeader {
        self.header.get().expect("published schema mutation")
    }
    pub fn receipt(&self, outcome: DefinitionOutcome) -> MutationReceipt {
        self.mutation().complete(MutationValue::Schema {
            position: self.value.position.clone(),
            key: self.value.key.clone(),
            outcome,
        })
    }
    pub fn replay(&self, receipt: &MutationReceipt) -> Result<()> {
        if receipt.mutation != self.mutation().mutation || receipt.fingerprint != self.mutation().fingerprint {
            return Err(Error::backend(
                "SetSchema",
                crate::BackendFailure::Conflict,
                std::io::Error::other("mutation identity has a different payload"),
            ));
        }
        match &receipt.value {
            MutationValue::Schema { position, key, outcome } if position == &self.value.position && key == &self.value.key => {
                outcome.validate()?;
                if matches!(outcome, DefinitionOutcome::Applied { revision } if *revision != position.sequence) {
                    return Err(Error::InvalidInput {
                        field: "SetSchema",
                        reason: "provider returned a different revision",
                    });
                }
                Ok(())
            }
            _ => Err(Error::InvalidInput {
                field: "SetSchema",
                reason: "mutation owns a different result",
            }),
        }
    }
}
impl StorageManager {
    pub async fn set_schema_once(
        &self, mutation: &MutationId, key: SchemaKey, schema: impl Into<Arc<crate::schema::graph::Schema>>, expected: Option<StorageCursor>,
    ) -> Result<StorageCursor> {
        let schema = schema.into();
        schema.prepare(&harmos_domain::config::SchemaSettings::default()).map_err(|_| Error::InvalidInput {
            field: "SetSchema",
            reason: "invalid finite schema graph",
        })?;
        if let Some(expected) = &expected {
            self.inner.primary.get_history().check(&expected.history)?;
            if expected.sequence == 0 {
                return Err(Error::InvalidInput {
                    field: "SetSchema",
                    reason: "zero expected revision",
                });
            }
        }
        let base = crate::admissions::schema_mutation(mutation, &key, &schema)?;
        let mut hash = Sha256::new();
        hash.update(b"storage.set_schema:v1\0");
        hash.update(base.fingerprint);
        hash.update(serde_json::to_vec(&expected).map_err(|_| Error::InvalidInput {
            field: "SetSchema",
            reason: "invalid expected revision",
        })?);
        let fingerprint: [u8; 32] = hash.finalize().into();
        let bytes = StorageSchema::new(self.position(u64::MAX), key.clone(), schema.clone()).encoded_bytes()?;
        let (_, completion) = self
            .admit_mutation_owned(
                crate::manager::Domain::Data,
                bytes,
                1,
                Some(crate::admissions::Mutation { id: mutation, fingerprint }),
                false,
                |sequence| {
                    Ok(crate::manager::Operation::SetSchema(Arc::new(SchemaExchange {
                        value: StorageSchema::new(self.position(sequence), key, schema),
                        expected,
                        header: OnceLock::new(),
                    })))
                },
            )
            .await?;
        let completion = completion.expect("schema admission owns completion");
        self.inner
            .wait(None, |state| match state.receipts.lookup(mutation, &fingerprint, Some(&completion)) {
                Ok(ReceiptState::Complete(MutationValue::Schema { position, outcome, .. })) => {
                    match outcome.into_result(DefinitionKind::Schema) {
                        Ok(_) => crate::capacity::Attempt::Ready(position),
                        Err(error) => crate::capacity::Attempt::Refuse(crate::capacity::Refusal::Failed(error)),
                    }
                }
                Ok(ReceiptState::Pending) => {
                    if let Some(error) = &state.status.failure {
                        crate::capacity::Attempt::Refuse(crate::capacity::Refusal::Failed(Error::Persistence { source: error.clone() }))
                    } else {
                        crate::capacity::Attempt::Wait
                    }
                }
                Ok(_) => crate::capacity::Attempt::Refuse(crate::capacity::Refusal::Failed(Error::InvalidInput {
                    field: "SetSchema",
                    reason: "mutation owns a different result",
                })),
                Err(error) => crate::capacity::Attempt::Refuse(crate::capacity::Refusal::Failed(error)),
            })
            .await
            .map_err(crate::capacity::Refusal::into_error)
    }
}
