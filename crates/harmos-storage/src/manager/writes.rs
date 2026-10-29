/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

impl<C> StorageManager<C> {
    #[tracing::instrument(name = "storage.manager.writes.write_schema", level = "debug", skip_all)]
    pub async fn write_schema(&self, key: SchemaKey, schema: impl Into<Arc<crate::schema::graph::Schema>>) -> Result<StorageCursor> {
        self.admit_schema(key, schema.into(), None).await
    }

    #[tracing::instrument(name = "storage.manager.writes.admit_schema", level = "debug", skip_all)]
    pub(crate) async fn admit_schema(
        &self, key: SchemaKey, schema: Arc<crate::schema::graph::Schema>, mutation: Option<&crate::storage::MutationId>,
    ) -> Result<StorageCursor> {
        let mutation = mutation.map(|id| crate::admissions::schema_mutation(id, &key, &schema)).transpose()?;
        // Replay Completed Admission Before Later I/O Errors.
        // Admission Rechecks The Same Ledger After Every Suspension.
        if let Some(mutation) = mutation {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if !state.ready || !state.status.running {
                return Err(Error::NotReady { domain: "storage" });
            }
            if let ReceiptState::Complete(value) = state.receipts.lookup(mutation.id, &mutation.fingerprint, None)? {
                return value.cursor();
            }
        }
        schema.prepare(&harmos_domain::config::SchemaSettings::default()).map_err(|_| Error::InvalidInput {
            field: "storage schema",
            reason: "invalid message fields",
        })?;
        let bytes = StorageSchema::new(self.position(u64::MAX), key.clone(), schema.clone()).encoded_bytes()?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, 1, mutation, false, |sequence| {
                Ok(Operation::Schema(StorageBatch::new(vec![StorageSchema::new(self.position(sequence), key, schema)])))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub fn stream_batch_len(&self, rows: &[StreamInput]) -> Result<usize> {
        self.batch_prefix(rows.iter().map(|row| {
            let value = StoredRow {
                record: StoredRecord {
                    position: self.position(u64::MAX),
                    schema: row.record.schema.clone(),
                    scope: row.record.scope.clone(),
                    item: row.record.item.clone(),
                    source: row.record.source.clone(),
                    source_time: row.record.source_time,
                    payload: row.record.payload.as_slice(),
                },
                stream: row.stream.clone(),
                sequence: row.sequence,
            };
            encoded_size(&value)
        }))
    }

    pub(super) fn batch_prefix(&self, sizes: impl Iterator<Item = Result<usize>>) -> Result<usize> {
        let mut bytes = 0usize;
        let mut rows = 0usize;
        let maximum = self.inner.settings.max_batch_bytes().get();
        for size in sizes {
            let size = size?;
            if size > maximum {
                return if rows == 0 { Err(Error::ResourceExhausted { resource: "oversized atomic write row" }) } else { Ok(rows) };
            }
            if size > maximum - bytes {
                break;
            }
            bytes += size;
            rows += 1;
        }
        Ok(rows)
    }

    #[tracing::instrument(name = "storage.manager.writes.write_records", level = "debug", skip_all)]
    pub async fn write_records(&self, records: Vec<RecordInput>) -> Result<StorageCursor> {
        self.admit_records(records, None).await
    }

    #[tracing::instrument(name = "storage.manager.writes.admit_records", level = "debug", skip_all)]
    pub(crate) async fn admit_records(&self, records: Vec<RecordInput>, mutation: Option<Mutation<'_>>) -> Result<StorageCursor> {
        if records.is_empty() {
            return Ok(self.status().accepted);
        }
        let bytes = records.iter().try_fold(0usize, |bytes, record| {
            bytes.checked_add(record.encoded_bytes(self.inner.primary.history())?).ok_or(Error::InvalidInput {
                field: "storage admission",
                reason: "size overflow",
            })
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, records.len(), mutation, false, |first| {
                Ok(Operation::Records(StorageBatch::new(
                    records.into_iter().enumerate().map(|(index, record)| record.stored(self.position(first + index as u64))).collect(),
                )))
            })
            .await?;
        Ok(self.position(sequence))
    }

    #[tracing::instrument(name = "storage.manager.writes.write_stream", level = "debug", skip_all)]
    pub async fn write_stream(&self, rows: Vec<StreamInput>) -> Result<StorageCursor> {
        self.admit_stream(rows, None).await
    }

    #[tracing::instrument(name = "storage.manager.writes.admit_stream", level = "debug", skip_all)]
    pub(crate) async fn admit_stream(&self, rows: Vec<StreamInput>, mutation: Option<Mutation<'_>>) -> Result<StorageCursor> {
        if rows.is_empty() {
            return Ok(self.status().accepted);
        }
        let bytes = rows.iter().try_fold(0usize, |bytes, row| {
            bytes
                .checked_add(row.record.encoded_bytes(self.inner.primary.history())?)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<StoredRow>() - std::mem::size_of::<StoredRecord>()))
                .and_then(|bytes| bytes.checked_add(row.stream.as_str().len()))
                .ok_or(Error::InvalidInput {
                    field: "storage admission",
                    reason: "size overflow",
                })
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, rows.len(), mutation, true, |first| {
                Ok(Operation::Stream(StorageBatch::new(
                    rows.into_iter()
                        .enumerate()
                        .map(|(index, row)| StoredRow {
                            record: row.record.stored(self.position(first + index as u64)),
                            stream: row.stream,
                            sequence: row.sequence,
                        })
                        .collect(),
                )))
            })
            .await?;
        Ok(self.position(sequence))
    }
}
