/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl<C> StorageManager<C> {
    pub async fn write_schema(&self, key: &SchemaKey, schema: &crate::schema::graph::Schema) -> Result<StorageCursor> {
        self.admit_schema(key, schema, None).await
    }

    pub(crate) async fn admit_schema(
        &self, key: &SchemaKey, schema: &crate::schema::graph::Schema, mutation: Option<&crate::storage::MutationId>,
    ) -> Result<StorageCursor> {
        let scratch = schema
            .validation_memory_bytes(&harmos_domain::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput { field: "storage schema", reason: "invalid message field budget" })?;
        let mutation = mutation.map(|id| crate::admissions::schema_mutation(id, key, schema)).transpose()?;
        // Replay Completed Admission Before Scratch Pressure Or Later I/O Errors.
        // Admission Rechecks The Same Ledger After Every Suspension.
        if let Some(mutation) = mutation {
            let state = self.inner.state.lock().unwrap();
            if state.status.closing {
                return Err(Error::Closing { domain: "storage" });
            }
            if !state.ready || !state.status.running {
                return Err(Error::NotReady { domain: "storage" });
            }
            if let ReceiptState::Complete(value) = state.receipts.lookup(mutation.id, &mutation.fingerprint)? {
                return value.cursor();
            }
        }
        let scratch = self.reserve_scratch(scratch).await?;
        schema
            .prepare(&harmos_domain::config::SchemaSettings::default())
            .map_err(|_| Error::InvalidInput { field: "storage schema", reason: "invalid message fields" })?;
        drop(scratch);
        let bytes = StorageSchema::resident_bytes(key, schema, self.inner.primary.history())?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, 1, mutation, false, |sequence| {
                Ok(Operation::Schema(StorageSchema::new(self.position(sequence), key.clone(), schema.clone())))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub fn record_batch_len(&self, records: &[RecordWrite<'_>]) -> Result<usize> {
        self.batch_prefix(records.iter().map(|record| {
            let value = StoredRecord {
                position: self.position(u64::MAX),
                schema: record.schema.clone(),
                scope: record.scope.cloned(),
                item: record.item.clone(),
                source: record.source.cloned(),
                source_time: record.source_time,
                payload: record.payload,
            };
            Ok((encoded_size(&value)?, record.resident_bytes(self.inner.primary.history())?))
        }))
    }

    pub fn stream_batch_len(&self, rows: &[StreamWrite<'_>]) -> Result<usize> {
        self.batch_prefix(rows.iter().map(|row| {
            let value = StoredRow {
                record: StoredRecord {
                    position: self.position(u64::MAX),
                    schema: row.record.schema.clone(),
                    scope: row.record.scope.cloned(),
                    item: row.record.item.clone(),
                    source: row.record.source.cloned(),
                    source_time: row.record.source_time,
                    payload: row.record.payload,
                },
                stream: row.stream.clone(),
                sequence: row.sequence,
            };
            let resident = row
                .record
                .resident_bytes(self.inner.primary.history())?
                .checked_add(std::mem::size_of::<StoredRow>() - std::mem::size_of::<StoredRecord>())
                .and_then(|bytes| bytes.checked_add(row.stream.as_str().len()))
                .ok_or(Error::ResourceExhausted { resource: "stream batch memory" })?;
            Ok((encoded_size(&value)?, resident))
        }))
    }

    pub(super) fn batch_prefix(&self, sizes: impl Iterator<Item = Result<(usize, usize)>>) -> Result<usize> {
        let mut bytes = 0usize;
        let mut rows = 0usize;
        let mut resident = 0usize;
        let memory = self.inner.settings.request.max_pending_write_bytes.get();
        let maximum = self.inner.settings.max_batch_bytes().get();
        for size in sizes {
            let (size, held) = size?;
            if size > maximum || held > memory {
                return if rows == 0 { Err(Error::ResourceExhausted { resource: "oversized atomic write row" }) } else { Ok(rows) };
            }
            if size > maximum - bytes || held > memory - resident {
                break;
            }
            bytes += size;
            resident += held;
            rows += 1;
        }
        Ok(rows)
    }

    pub async fn write_records(&self, records: &[RecordWrite<'_>]) -> Result<StorageCursor> {
        self.admit_records(records.iter().copied(), None).await
    }

    pub(crate) async fn admit_records<'a>(
        &self, records: impl ExactSizeIterator<Item = RecordWrite<'a>> + Clone, mutation: Option<Mutation<'_>>,
    ) -> Result<StorageCursor> {
        if records.len() == 0 {
            return Ok(self.status().accepted);
        }
        let bytes = records.clone().try_fold(0usize, |bytes, record| {
            bytes
                .checked_add(record.resident_bytes(self.inner.primary.history())?)
                .ok_or(Error::InvalidInput { field: "storage admission", reason: "size overflow" })
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, records.len(), mutation, false, |first| {
                Ok(Operation::Records(
                    records.enumerate().map(|(index, record)| record.stored(self.position(first + index as u64))).collect(),
                ))
            })
            .await?;
        Ok(self.position(sequence))
    }

    pub async fn write_stream(&self, rows: &[StreamWrite<'_>]) -> Result<StorageCursor> {
        self.admit_stream(rows.iter().copied(), None).await
    }

    pub(crate) async fn admit_stream<'a>(
        &self, rows: impl ExactSizeIterator<Item = StreamWrite<'a>> + Clone, mutation: Option<Mutation<'_>>,
    ) -> Result<StorageCursor> {
        if rows.len() == 0 {
            return Ok(self.status().accepted);
        }
        let bytes = rows.clone().try_fold(0usize, |bytes, row| {
            bytes
                .checked_add(row.record.resident_bytes(self.inner.primary.history())?)
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<StoredRow>() - std::mem::size_of::<StoredRecord>()))
                .and_then(|bytes| bytes.checked_add(row.stream.as_str().len()))
                .ok_or(Error::InvalidInput { field: "storage admission", reason: "size overflow" })
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, bytes, rows.len(), mutation, true, |first| {
                Ok(Operation::Stream(
                    rows.enumerate()
                        .map(|(index, row)| StoredRow {
                            record: row.record.stored(self.position(first + index as u64)),
                            stream: row.stream.clone(),
                            sequence: row.sequence,
                        })
                        .collect(),
                ))
            })
            .await?;
        Ok(self.position(sequence))
    }
}
