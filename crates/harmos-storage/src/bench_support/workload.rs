/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Backend-independent, deterministic storage benchmark inputs and query plans.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

// Third Party Imports

// Local Imports
use crate::query::{
    columnar::{ArrowScan, ColumnKind, ColumnSource},
    index::{IndexDefinition, IndexRead},
    projection::{ProjectionDefinition, ProjectionTransform},
    time_buckets::TimeBucketPlan,
};
use crate::*;
use harmos_domain::schema::Described;

/* === Constants === */

pub const ROWS: usize = 2_000_000;
pub const CHUNK_ROWS: usize = 16_384;
pub const WINDOW_ROWS: usize = 4096;
pub const SAMPLES: usize = 10;
pub const CONTENTION_ROUNDS: usize = 64;
pub const WRITERS: usize = 8;
const DAYS_NS: i64 = 3 * 24 * 60 * 60 * 1_000_000_000;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos_domain::Message, Clone, Debug, Default)]
pub struct Sample {
    pub time_ns: i64,
    pub plot: u32,
    pub value: f64,
}

/// The small form crosses exactly one physical chunk boundary.
pub struct Workload {
    pub rows: usize,
    pub history: StoreHistory,
    pub source: ColumnSource,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Workload {
    pub fn new(rows: usize) -> Self {
        Self {
            rows,
            history: StoreHistory {
                store: Identifier::new("multi-day").unwrap(),
                epoch: Identifier::new("native").unwrap(),
            },
            source: ColumnSource {
                kind: ColumnKind::Records,
                schema: SchemaKey {
                    id: Identifier::new("sample").unwrap(),
                    version: NonZeroU32::MIN,
                    payload_encoding: PayloadEncoding::PostcardV1,
                },
            },
        }
    }
    pub fn cut(&self) -> StorageCursor {
        self.position(self.rows as u64 + 1)
    }
    pub fn position(&self, sequence: u64) -> StorageCursor {
        StorageCursor { history: self.history.clone(), sequence }
    }
    pub fn schema(&self, position: u64) -> StorageSchema {
        let graph =
            crate::schema::graph::Schema::from_declaration(Sample::DEFINITION, &harmos_domain::config::SchemaSettings::default()).unwrap();
        StorageSchema::new(self.position(position), self.source.schema.clone(), graph)
    }
    pub fn records(&self, first: usize, end: usize) -> StorageBatch<StoredRecord> {
        StorageBatch::new((first..end).map(|index| self.record(index, index as u64 + 2, sample(index))).collect())
    }
    pub fn record(&self, identity: usize, position: u64, sample: Sample) -> StoredRecord {
        StoredRecord {
            position: self.position(position),
            schema: self.source.schema.clone(),
            scope: None,
            item: Identifier::new(format!("sample-{identity}")).unwrap(),
            source: None,
            source_time: None,
            payload: sample.encode(),
        }
    }
    pub fn scan(&self, offset: usize) -> ArrowScan {
        ArrowScan {
            source: self.source.clone(),
            columns: vec!["payload.value".into()],
            predicate: None,
            time: None,
            start: offset as u64 + 1,
            after: None,
            limits: self.page(),
            work: self.work(),
        }
    }
    pub fn page(&self) -> ReadLimits {
        ReadLimits {
            rows: nz(WINDOW_ROWS),
            bytes: nz(8 * 1024 * 1024),
        }
    }
    pub fn work(&self) -> ReadLimits {
        ReadLimits {
            rows: nz(self.rows + CHUNK_ROWS),
            bytes: nz(1024 * 1024 * 1024),
        }
    }
    pub fn projection(&self) -> ProjectionDefinition {
        ProjectionDefinition {
            name: Identifier::new("values").unwrap(),
            version: 1,
            source: self.source.clone(),
            transform: ProjectionTransform::Select(vec!["payload.value".into()]),
        }
    }
    pub fn buckets(&self) -> ProjectionDefinition {
        ProjectionDefinition {
            name: Identifier::new("buckets").unwrap(),
            version: 1,
            source: self.source.clone(),
            transform: ProjectionTransform::TimeBuckets(TimeBucketPlan {
                origin_ns: 0,
                width_ns: NonZeroU64::new(60 * 1_000_000_000).unwrap(),
                time: "payload.time_ns".into(),
                value: "payload.value".into(),
                identity: "_storage.item".into(),
                group: vec!["payload.plot".into()],
                max_samples: nz(self.rows + CHUNK_ROWS),
                max_bytes: nz(1024 * 1024 * 1024),
            }),
        }
    }
    pub fn index(&self) -> IndexDefinition {
        IndexDefinition {
            name: Identifier::new("by-value").unwrap(),
            version: 1,
            source: self.source.clone(),
            columns: vec!["payload.value".into()],
        }
    }
    pub fn index_read(&self) -> IndexRead {
        IndexRead {
            revision: None,
            name: self.index().name,
            lower: None,
            upper: None,
            prefix: vec![],
            descending: false,
            after: None,
            limits: self.page(),
            work: self.work(),
        }
    }
    pub fn exchange(&self, sequence: u64, expected: Option<ObjectVersionToken>, value: u64) -> ObjectExchange {
        let schema = self.schema(sequence - 1);
        let position = self.position(sequence);
        ObjectExchange {
            expected,
            schema,
            value: ObjectEntry {
                version: ObjectVersionToken::new(position.clone()),
                position,
                value: ObjectValue {
                    schema: self.source.schema.clone(),
                    item: ObjectKey::new("counter", self.page().bytes).unwrap(),
                    payload: Sample { value: value as f64, ..sample(0) }.encode(),
                },
            },
        }
    }
}

/* === Public Functions === */

pub fn sample(index: usize) -> Sample {
    let logical = if index % 4096 == 4095 { index - 2048 } else { index };
    Sample {
        time_ns: (logical as i128 * i128::from(DAYS_NS) / ROWS as i128) as i64,
        plot: (logical % 24) as u32,
        value: ((logical as u64 * 7919) % 65521) as f64,
    }
}

/* === Private Functions === */

fn nz(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

/* === Tests === */
