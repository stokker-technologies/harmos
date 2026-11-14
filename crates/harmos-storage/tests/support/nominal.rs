/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Fixture Stream and File Read Stories over Balanced Three-Phase Samples

#![allow(dead_code, unused_imports)]

/* === Imports === */

pub(crate) use harmos_query::typed::StreamRead;

// Standard Library Imports
pub(crate) use std::time::{Duration, UNIX_EPOCH};
// Third Party Imports
pub(crate) use harmos_domain::model::storage::query::Query;
pub(crate) use harmos_query::QueryArrow;
pub(crate) use harmos_query::transform::{
    Transform,
    aggregate::{Aggregate, Measure},
    downsample::{Algorithm, Downsample},
    histogram::Histogram,
    project::{Project, Projection},
};
pub(crate) use harmos_storage::FileRead;
pub(crate) use harmos_storage::Identifier;
pub(crate) use harmos_storage::query::RowSlice;
pub(crate) use harmos_storage::query::Source;
pub(crate) use harmos_storage::query::StreamSelection;
pub(crate) use harmos_storage::*;
// Local Imports
pub(crate) use super::backend::storage_settings;
pub(crate) use super::waveforms::{self, Sample};

/* === Structs === */

// Fixture Storage Holding Durable Three-Phase Stream Samples.
pub(crate) struct Nominal {
    pub(crate) storage: StorageManager,
    pub(crate) schema: SchemaKey,
    pub(crate) samples: Vec<Sample>,
    pub(crate) durable: StorageCursor,
}

/* === Implementations === */

impl Nominal {
    pub(crate) async fn new() -> Self {
        // Configure And Start A Storage Fixture.
        let fixture = super::fixture::Fixture::new(storage_settings()).await;
        let storage = fixture.storage;

        // Store The Schema And Balanced Three-Phase Samples In Several Batches.
        let schema = waveforms::schema();
        storage.write_schema(schema.clone(), waveforms::definition()).await.unwrap();
        let samples = waveforms::samples(4);
        let inputs = waveforms::inputs(&samples);
        for inputs in inputs.chunks(192) {
            let batch: Vec<_> = inputs.to_vec();
            storage.append_samples((batch).to_vec()).await.unwrap();
        }
        let durable = storage.flush().await.unwrap();
        Self { storage, schema, samples, durable }
    }

    pub(crate) fn query(&self, selection: StreamSelection, transforms: Vec<Transform>) -> Query {
        Query {
            merge: None,
            at: self.durable.clone(),
            source: Source::Stream(selection),
            transforms,
            slice: RowSlice::default(),
        }
    }

    pub(crate) fn all(&self, after: u64) -> StreamSelection {
        StreamSelection {
            schema: self.schema.clone(),
            scope: None,
            item: None,
            stream: None,
            source_start: None,
            source_end: None,
            after,
        }
    }

    // One Half-Open Period Of The Experiment Scope.
    pub(crate) fn period(&self) -> StreamSelection {
        StreamSelection {
            scope: Some(Identifier::new("experiment").unwrap()),
            source_start: Some(UNIX_EPOCH + Duration::from_millis(64)),
            source_end: Some(UNIX_EPOCH + Duration::from_millis(128)),
            ..self.all(0)
        }
    }

    pub(crate) async fn finish(self) {
        self.storage.close().await.unwrap();
    }
}

/* === Private Functions === */

pub(crate) fn project(fields: &[(&str, &str)]) -> Transform {
    Transform::Project(Project {
        fields: fields
            .iter()
            .map(|(field, name)| Projection {
                field: (*field).into(),
                name: (*name).into(),
            })
            .collect(),
    })
}

pub(crate) struct QueryRead {
    pub(crate) batches: Vec<arrow_array::RecordBatch>,
    pub(crate) schema_first: bool,
    pub(crate) schemas: usize,
    pub(crate) completed_rows: u64,
}

pub(crate) async fn batches(storage: &StorageManager, query: Query) -> QueryRead {
    let mut reader = storage.query(query).await.unwrap();
    let mut read = QueryRead {
        batches: Vec::new(),
        schema_first: false,
        schemas: 0,
        completed_rows: 0,
    };
    let mut first = true;
    while let Some(output) = reader.next().await.unwrap() {
        match output {
            QueryArrow::Schema(_) => {
                read.schemas += 1;
                read.schema_first |= first;
            }
            QueryArrow::Batch(batch) => read.batches.push(batch.as_record_batch().clone()),
        }
        first = false;
    }
    read.completed_rows = reader.completion().unwrap().rows;
    read
}

pub(crate) fn positions(read: &QueryRead) -> Vec<(u64, u64)> {
    let mut rows = Vec::new();
    for batch in &read.batches {
        let position = column::<arrow_array::UInt64Array>(batch, "position");
        let ordinal = column::<arrow_array::UInt64Array>(batch, "ordinal");
        for row in 0..batch.num_rows() {
            rows.push((position.value(row), ordinal.value(row)));
        }
    }
    rows
}

pub(crate) struct FileReadout {
    pub(crate) bytes: Vec<u8>,
    pub(crate) offsets: Vec<(u64, u64)>,
    pub(crate) completed_bytes: u64,
}

pub(crate) async fn read_range(storage: &StorageManager, file: &FileReference, range: ByteRange) -> FileReadout {
    let mut reader = storage
        .read_file(FileRead {
            source: harmos_domain::model::storage::files::FileSource::Stored(file.clone()),
            range,
        })
        .await
        .unwrap();
    let mut bytes = Vec::new();
    let mut offsets = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        offsets.push((batch.offset(), range.offset + bytes.len() as u64));
        bytes.extend_from_slice(batch.bytes());
    }
    FileReadout {
        bytes,
        offsets,
        completed_bytes: reader.completion().unwrap().bytes,
    }
}

pub(crate) fn column<'a, T: 'static>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a T {
    batch.column_by_name(name).unwrap().as_any().downcast_ref::<T>().unwrap()
}

pub(crate) fn rms() -> f64 {
    waveforms::AMPLITUDE / 2.0_f64.sqrt()
}
