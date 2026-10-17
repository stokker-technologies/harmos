/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Journaled Stream And File Read Stories Over Balanced Three-Phase Samples

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
// Third Party Imports
use harmos::storage::operations::{Query, ReadFile};
use harmos::storage::query::transform::{
    Transform,
    aggregate::{Aggregate, Measure},
    downsample::{Algorithm, Downsample},
    histogram::Histogram,
    project::{Project, Projection},
};
use harmos::storage::query::{QueryArrow, RowSlice, Source, StreamSelection};
use harmos::storage::*;
use harmos::{Identifier, Runtime};
// Local Imports
use super::backend::{Data, Recovery, history, storage_settings};
use super::support::{Bank, Ledger, catalog};
use super::waveforms::{self, Sample};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// Journaled Storage Holding Durable Three-Phase Stream Samples.
struct Nominal {
    _directory: tempfile::TempDir,
    storage: StorageManager,
    runtime: Runtime<Bank>,
    schema: SchemaKey,
    samples: Vec<Sample>,
    durable: StorageCursor,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Nominal {
    async fn new() -> Self {
        // Configure And Start A Real Temporary Journal.
        let directory = tempfile::tempdir().unwrap();
        let data = Arc::new(Data::<()>::journaled(directory.path(), history()).unwrap());
        let storage: StorageManager = StorageManager::new(
            StorageBackends {
                primary: data,
                state: Arc::new(Recovery::new(history())),
            },
            storage_settings(),
        )
        .unwrap();
        let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
            .register(catalog())
            .runtime_with_storage(&storage)
            .await
            .unwrap();

        // Store The Schema And Balanced Three-Phase Samples In Several Batches.
        let schema = waveforms::schema();
        storage
            .write_schema(&schema, &waveforms::definition())
            .await
            .unwrap();
        let samples = waveforms::samples(4);
        let inputs = waveforms::inputs(&samples);
        for inputs in inputs.chunks(192) {
            let batch: Vec<_> = inputs.iter().map(StreamInput::borrowed).collect();
            storage.write_stream(&batch).await.unwrap();
        }
        let durable = storage.flush().await.unwrap();
        Self {
            _directory: directory,
            storage,
            runtime,
            schema,
            samples,
            durable,
        }
    }

    fn query(&self, selection: StreamSelection, transforms: Vec<Transform>) -> Query {
        Query {
            at: self.durable.clone(),
            source: Source::Stream(selection),
            transforms,
            slice: RowSlice::default(),
        }
    }

    fn all(&self, after: u64) -> StreamSelection {
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
    fn period(&self) -> StreamSelection {
        StreamSelection {
            scope: Some(Identifier::new("experiment").unwrap()),
            source_start: Some(UNIX_EPOCH + Duration::from_millis(64)),
            source_end: Some(UNIX_EPOCH + Duration::from_millis(128)),
            ..self.all(0)
        }
    }

    async fn finish(self) {
        self.storage.close().await.unwrap();
        self.runtime.stop().await.unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn project(fields: &[(&str, &str)]) -> Transform {
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

// Every Data Batch A Query Delivers, In Order.
async fn batches(storage: &StorageManager, query: Query) -> Vec<arrow_array::RecordBatch> {
    let mut reader = storage.query(query).await.unwrap();
    assert!(matches!(
        reader.next().await.unwrap(),
        Some(QueryArrow::Schema(_))
    ));
    let mut batches = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        let QueryArrow::Batch(batch) = output else {
            panic!("schema must be emitted once");
        };
        batches.push(batch.as_record_batch().clone());
    }
    let rows: usize = batches.iter().map(|batch| batch.num_rows()).sum();
    assert_eq!(reader.completion().unwrap().rows, rows as u64);
    batches
}

// Stored Position And Ordinal Of Every Projected Row, In Delivery Order.
async fn positions(storage: &StorageManager, query: Query) -> Vec<(u64, u64)> {
    let mut rows = Vec::new();
    for batch in batches(storage, query).await {
        let position = column::<arrow_array::UInt64Array>(&batch, "position");
        let ordinal = column::<arrow_array::UInt64Array>(&batch, "ordinal");
        for row in 0..batch.num_rows() {
            rows.push((position.value(row), ordinal.value(row)));
        }
    }
    rows
}

async fn read_range(storage: &StorageManager, file: &FileReference, range: ByteRange) -> Vec<u8> {
    let mut reader = storage
        .read_file(ReadFile {
            file: file.clone(),
            range,
        })
        .await
        .unwrap();
    let mut bytes = Vec::new();
    while let Some(batch) = reader.next().await.unwrap() {
        assert_eq!(batch.offset, range.offset + bytes.len() as u64);
        bytes.extend_from_slice(&batch.bytes);
    }
    assert_eq!(reader.completion().unwrap().bytes, bytes.len() as u64);
    bytes
}

fn column<'a, T: 'static>(batch: &'a arrow_array::RecordBatch, name: &str) -> &'a T {
    batch
        .column_by_name(name)
        .unwrap()
        .as_any()
        .downcast_ref::<T>()
        .unwrap()
}

fn rms() -> f64 {
    waveforms::AMPLITUDE / 2.0_f64.sqrt()
}

/* === Tests === */
#[tokio::test]
async fn stream_values_roundtrip() {
    // Persisted Stream Values Match The Generator And Analytical RMS.
    let n = Nominal::new().await;
    let projection = project(&[
        ("payload.phase", "phase"),
        ("payload.ordinal", "ordinal"),
        ("payload.value", "value"),
    ]);
    let mut received = Vec::new();
    for batch in batches(&n.storage, n.query(n.all(0), vec![projection])).await {
        let phase = column::<arrow_array::UInt32Array>(&batch, "phase");
        let ordinal = column::<arrow_array::UInt64Array>(&batch, "ordinal");
        let value = column::<arrow_array::Float64Array>(&batch, "value");
        for row in 0..batch.num_rows() {
            received.push(Sample {
                phase: phase.value(row),
                ordinal: ordinal.value(row),
                value: value.value(row),
            });
        }
    }
    assert_eq!(received, n.samples);
    for phase in 0..3 {
        let values: Vec<_> = received
            .iter()
            .filter(|sample| sample.phase == phase)
            .map(|sample| sample.value)
            .collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let measured =
            (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt();
        assert!(mean.abs() < 1e-12);
        assert!((measured - rms()).abs() < 1e-12);
    }
    n.finish().await;
}

#[tokio::test]
async fn stream_resumes_after() {
    // Resuming After A Stored Position Yields Exactly The Later Rows, In Order.
    let n = Nominal::new().await;
    let positioned = |after| {
        n.query(
            n.all(after),
            vec![project(&[
                ("_storage.position", "position"),
                ("payload.ordinal", "ordinal"),
            ])],
        )
    };
    let all = positions(&n.storage, positioned(0)).await;
    assert_eq!(all.len(), n.samples.len());
    let middle = all[all.len() / 2].0;
    let resumed = positions(&n.storage, positioned(middle)).await;
    assert_eq!(resumed, all[all.len() / 2 + 1..]);
    assert!(resumed.windows(2).all(|pair| pair[0].0 < pair[1].0));
    n.finish().await;
}

#[tokio::test]
async fn period_aggregates_rms() {
    // One Half-Open Period Aggregates Every Selected Sample Per Phase.
    let n = Nominal::new().await;
    let aggregate = Transform::Aggregate(Aggregate {
        groups: vec![Projection {
            field: "payload.phase".into(),
            name: "phase".into(),
        }],
        measures: vec![Measure {
            field: "payload.value".into(),
            statistic: harmos::Aggregate::Rms,
            name: "rms".into(),
        }],
    });
    let mut phases = 0;
    for batch in batches(&n.storage, n.query(n.period(), vec![aggregate])).await {
        for value in column::<arrow_array::Float64Array>(&batch, "rms").values() {
            assert!((value - rms()).abs() < 1e-12);
            phases += 1;
        }
    }
    assert_eq!(phases, 3);
    n.finish().await;
}

#[tokio::test]
async fn histogram_counts_inputs() {
    // Histograms Count Every Selected Input Into Closed-Ended Bins.
    let n = Nominal::new().await;
    let histogram = Transform::Histogram(Histogram {
        field: "payload.value".into(),
        edges: vec![-12.0, -6.0, 0.0, 6.0, 12.0],
    });
    let mut counts = Vec::new();
    for batch in batches(&n.storage, n.query(n.period(), vec![histogram])).await {
        counts.extend_from_slice(column::<arrow_array::UInt64Array>(&batch, "count").values());
    }
    assert_eq!(
        counts.iter().sum::<u64>(),
        (waveforms::SAMPLES_PER_PERIOD * 3) as u64
    );
    let expected: Vec<u64> = [
        (-f64::INFINITY, -12.0),
        (-12.0, -6.0),
        (-6.0, 0.0),
        (0.0, 6.0),
        (6.0, 12.0),
        (12.0, f64::INFINITY),
    ]
    .into_iter()
    .enumerate()
    .map(|(bin, (lower, upper))| {
        n.samples
            .iter()
            .filter(|sample| (64..128).contains(&sample.ordinal))
            .filter(|sample| match bin {
                4 => sample.value >= lower && sample.value <= upper,
                5 => sample.value > lower,
                _ => sample.value >= lower && sample.value < upper,
            })
            .count() as u64
    })
    .collect();
    assert_eq!(counts, expected);
    n.finish().await;
}

#[tokio::test]
async fn downsample_bounds_rows() {
    // Display Downsampling Delivers A Bounded, Non-Empty Plan.
    let n = Nominal::new().await;
    let downsample = Transform::Downsample(Downsample {
        max_rows: 32,
        algorithm: Algorithm::Extrema,
        axis: None,
        value: Some("payload.value".into()),
    });
    let displayed: usize = batches(&n.storage, n.query(n.period(), vec![downsample]))
        .await
        .iter()
        .map(|batch| batch.num_rows())
        .sum();
    assert!(displayed > 0 && displayed <= 32);
    n.finish().await;
}

#[tokio::test]
async fn file_ranges_read() {
    // Empty, Small And Multi-Batch Files Use Deterministic Binary Contents.
    let n = Nominal::new().await;
    let batch_bytes = storage_settings().response.batch_bytes.get();
    for length in [0, 17, batch_bytes * 2 + 13] {
        let expected = waveforms::binary(length);
        let metadata = FileMetadata {
            id: Identifier::new(format!("binary-{length}")).unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(length as u64),
            expected_sha256: None,
        };
        let reference = n.storage.write_file(metadata, &expected).await.unwrap();
        let range = |offset, limit| ByteRange { offset, limit };
        assert_eq!(read_range(&n.storage, &reference, range(0, None)).await, expected);

        // Finished References Support Checked Half-Open Byte Ranges.
        let offset = reference.length.min(3);
        let selected = read_range(&n.storage, &reference, range(offset, Some(7))).await;
        assert_eq!(
            selected,
            expected[offset as usize..expected.len().min(offset as usize + 7)]
        );
        let end = read_range(&n.storage, &reference, range(reference.length, None)).await;
        assert!(end.is_empty());
        let beyond = n
            .storage
            .read_file(ReadFile {
                file: reference.clone(),
                range: range(reference.length + 1, None),
            })
            .await;
        assert!(beyond.is_err());
    }
    n.finish().await;
}
