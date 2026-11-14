/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::{app::SchematicEditor, streams::NetVoltage};
use harmos_runtime::storage::{Batch, Cursor, PayloadEncoding, RecordInput, SchemaKey, StorageManager, StreamConsumer, StreamInput};
use harmos_runtime::streams::{Definition, StreamRow};
use harmos_runtime::{Identifier, Message, Setup, schema::Described};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/* === Structs === */

#[derive(harmos_runtime::Message, Clone)]
pub(crate) struct SavedVoltage {
    pub position: u64,
    pub gap_missed: Option<u64>,
    pub net: u64,
    pub sampled_at_us: u64,
    pub voltage_mv: i32,
}

struct Recordings {
    storage: StorageManager,
    cursor: Cursor,
    pending_gap: u64,
    schema_written: bool,
    failed: bool,
}

/* === Implementations === */

impl StreamConsumer<SchematicEditor> for Recordings {
    fn resume(&self) -> Cursor {
        self.cursor
    }
    fn selections(&self) -> Vec<Definition> {
        vec![NetVoltage::ENTRY.build().expect("declared stream is valid")]
    }
    async fn drain(&mut self, batch: Batch<SchematicEditor>) -> Cursor {
        if self.failed {
            return self.cursor;
        }
        let rows = batch.select::<NetVoltage>().collect::<Vec<_>>();
        self.pending_gap =
            self.pending_gap.saturating_add(batch.reaches().0.saturating_sub(self.cursor.0).saturating_sub(rows.len() as u64));
        let mut values = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let value = SavedVoltage {
                position: row.position.0,
                gap_missed: (index == 0 && self.pending_gap > 0).then_some(self.pending_gap),
                net: u64::from(row.value.net.0),
                sampled_at_us: row.value.sampled_at_us,
                voltage_mv: row.value.voltage_mv,
            };
            values.push(StreamInput {
                record: RecordInput {
                    schema: voltage_schema(),
                    scope: None,
                    item: Identifier::new(format!("net-{}", value.net)).unwrap(),
                    source: None,
                    source_time: Some(SystemTime::UNIX_EPOCH + Duration::from_micros(value.sampled_at_us)),
                    payload: value.encode(),
                },
                stream: Identifier::new("net_voltage").unwrap(),
                sequence: self.cursor.0 + index as u64 + 1,
            });
        }
        if !values.is_empty() {
            let result = async {
                if !self.schema_written {
                    let schema = harmos_runtime::schema::graph::Schema::from_declaration(
                        SavedVoltage::DEFINITION,
                        &harmos_domain::config::SchemaSettings::default(),
                    )
                    .map_err(|source| harmos_runtime::Error::OperationFailed {
                        operation: "declare recorded voltage schema".into(),
                        source: Arc::new(source),
                    })?;
                    self.storage.write_schema(voltage_schema(), schema).await?;
                    self.schema_written = true;
                }
                self.storage.append_samples(values).await?;
                self.storage.flush().await?;
                Ok::<(), harmos_runtime::Error>(())
            }
            .await;
            if let Err(error) = result {
                self.failed = true;
                tracing::error!(action = "record schematic stream", %error);
                return self.cursor;
            }
            self.pending_gap = 0;
        }
        self.cursor = batch.reaches();
        self.cursor
    }
}

/* === Public Functions === */

pub fn recordings(setup: &mut Setup<SchematicEditor>, storage: StorageManager) {
    setup.settings().attach_streams(Recordings {
        storage,
        cursor: Cursor::ORIGIN,
        pending_gap: 0,
        schema_written: false,
        failed: false,
    });
}

pub(crate) fn voltage_schema() -> SchemaKey {
    SchemaKey {
        id: Identifier::new("net-voltage").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    }
}
