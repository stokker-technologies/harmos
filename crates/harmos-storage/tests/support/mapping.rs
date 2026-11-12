/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */
#![allow(dead_code, unused_imports)]

/* === Imports === */

// Standard Library Imports
pub(crate) use std::num::{NonZeroU32, NonZeroUsize};

// Third Party Imports
pub(crate) use arrow_array::{Array, Float64Array, ListArray, StructArray, UInt32Array};
pub(crate) use harmos_storage::Query;
pub(crate) use harmos_storage::query::{QueryArrow, RecordSelection, RowSlice, Source};
pub(crate) use harmos_storage::*;
pub(crate) use harmos_storage::{Identifier, Message};

// Local Imports
pub(crate) use super::backend::storage_settings;

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug)]
pub(crate) struct Payload {
    label: Option<String>,
    values: Vec<f64>,
    event: Event,
}

#[derive(harmos_domain::Message, Clone, Debug, Default)]
pub(crate) struct Expansion {
    values: Vec<String>,
}

// Fixture Storage Holding Saved Records Of One Declared Schema.
pub(crate) struct Mapped {
    pub(crate) storage: StorageManager,
    pub(crate) data: std::sync::Arc<super::backend::Data>,
    schema: SchemaKey,
    at: StorageCursor,
}

/* === Enums === */

#[derive(harmos_domain::Message, Clone, Debug)]
pub(crate) enum Event {
    Reading(i64),
    Empty,
}

impl Default for Event {
    fn default() -> Self {
        Self::Reading(Default::default())
    }
}

/* === Implementations === */

impl Mapped {
    pub(crate) async fn new<T: harmos_domain::schema::Described + Message>(settings: StorageSettings, id: &str, payloads: &[T]) -> Self {
        // Start The Existing Storage Fixture.
        let fixture = super::fixture::Fixture::new(settings).await;
        let storage = fixture.storage;

        // Declare The Schema And Save One Record Per Payload.
        let schema = SchemaKey {
            id: Identifier::new(id).unwrap(),
            version: NonZeroU32::MIN,
            payload_encoding: PayloadEncoding::PostcardV1,
        };
        let graph =
            harmos_domain::schema::graph::Schema::from_declaration(T::DEFINITION, &harmos_domain::config::SchemaSettings::default())
                .unwrap();
        storage.write_schema(schema.clone(), graph.clone()).await.unwrap();
        for (chunk, payloads) in payloads.chunks(4096).enumerate() {
            let records = payloads
                .iter()
                .enumerate()
                .map(|(index, payload)| {
                    let item = Identifier::new(format!("row-{}", chunk * 4096 + index)).unwrap();
                    RecordInput {
                        schema: schema.clone(),
                        scope: None,
                        item: item.clone(),
                        source: None,
                        source_time: None,
                        payload: payload.encode(),
                    }
                })
                .collect();
            storage.write_records(records).await.unwrap();
        }
        let at = storage.flush().await.unwrap();
        Self { storage, data: fixture.data, schema, at }
    }

    pub(crate) fn query(&self, item: Option<&str>) -> Query {
        let item = item.map(|item| {
            let identifier = Identifier::new(item);
            identifier.unwrap()
        });
        Query {
            at: self.at.clone(),
            source: Source::Records(RecordSelection {
                schema: self.schema.clone(),
                scope: None,
                item,
                source_start: None,
                source_end: None,
            }),
            transforms: vec![],
            slice: RowSlice { offset: 0, limit: None },
        }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
    }
}

/* === Private Functions === */

pub(crate) fn payloads() -> [Payload; 2] {
    [
        Payload {
            label: None,
            values: vec![0.0, -1.0, 2.0],
            event: Event::Reading(-7),
        },
        Payload {
            label: Some("second".into()),
            values: vec![],
            event: Event::Empty,
        },
    ]
}

pub(crate) fn compact() -> [Expansion; 1] {
    [Expansion { values: vec![String::new(); 4096] }]
}

/* === Tests === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
pub(crate) struct PlotSample {
    pub(crate) x: f64,
    pub(crate) y: f64,
}
