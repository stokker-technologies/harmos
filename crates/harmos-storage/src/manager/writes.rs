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

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StorageManager {
    #[tracing::instrument(name = "storage.manager.writes.write_schema", level = "debug", skip_all)]
    pub async fn write_schema(&self, key: SchemaKey, schema: impl Into<Arc<crate::schema::graph::Schema>>) -> Result<StorageCursor> {
        self.admit_schema(key, schema.into()).await
    }

    #[tracing::instrument(name = "storage.manager.writes.admit_schema", level = "debug", skip_all)]
    pub(crate) async fn admit_schema(&self, key: SchemaKey, schema: Arc<crate::schema::graph::Schema>) -> Result<StorageCursor> {
        schema.prepare(&harmos_domain::config::SchemaSettings::default()).map_err(|_| Error::InvalidInput {
            field: "storage schema",
            reason: "invalid message fields",
        })?;
        let sequence = self
            .admit_mutation(Domain::Data, 1, None, false, |sequence| {
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

    #[tracing::instrument(name = "storage.manager.writes.append_records", level = "debug", skip_all)]
    pub async fn append_records(&self, records: Vec<RecordInput>) -> Result<StorageCursor> {
        self.admit_records(records, None).await
    }

    #[tracing::instrument(name = "storage.manager.writes.admit_records", level = "debug", skip_all)]
    pub(crate) async fn admit_records(&self, records: Vec<RecordInput>, mutation: Option<Mutation<'_>>) -> Result<StorageCursor> {
        if records.is_empty() {
            return Ok(self.status().accepted);
        }
        let sequence = self
            .admit_mutation(Domain::Data, records.len(), mutation, false, |first| {
                Ok(Operation::Records(StorageBatch::new(
                    records.into_iter().enumerate().map(|(index, record)| record.stored(self.position(first + index as u64))).collect(),
                )))
            })
            .await?;
        Ok(self.position(sequence))
    }

    #[tracing::instrument(name = "storage.manager.writes.append_samples", level = "debug", skip_all)]
    pub async fn append_samples(&self, rows: Vec<StreamInput>) -> Result<StorageCursor> {
        self.admit_stream(rows, None).await
    }

    #[tracing::instrument(name = "storage.manager.writes.admit_stream", level = "debug", skip_all)]
    pub(crate) async fn admit_stream(&self, rows: Vec<StreamInput>, mutation: Option<Mutation<'_>>) -> Result<StorageCursor> {
        if rows.is_empty() {
            return Ok(self.status().accepted);
        }
        let sequence = self
            .admit_mutation(Domain::Data, rows.len(), mutation, true, |first| {
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
