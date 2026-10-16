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
use super::backend::{Data, Recovery, history, storage_settings};
use super::support::{Bank, Ledger, catalog};
use super::waveforms::{self, Experiment, Objects, Sample};
use harmos::storage::query::transform::{
    Transform,
    aggregate::{Aggregate, Measure},
    downsample::{Algorithm, Downsample},
    histogram::Histogram,
    project::{Project, Projection},
};
use harmos::storage::query::{Query, QueryOutput, RowSlice, Source, StreamSelection};
use harmos::storage::*;
use harmos::{Identifier, Runtime};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

/* === Constants === */
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
/* === Enums === */
/* === Traits === */
/* === Implementations === */
/* === Public Functions === */
/* === Private Functions === */
async fn read_range<C>(
    storage: &StorageManager<C>,
    file: &FileReference,
    range: ByteRange,
) -> Vec<u8> {
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
/* === Tests === */
#[tokio::test]
async fn nominal() {
    // Configure And Start A Real Temporary Journal.
    let directory = tempfile::tempdir().unwrap();
    let identity = history();
    let settings = storage_settings();
    let data = Arc::new(Data::<Objects>::journaled(directory.path(), identity.clone()).unwrap());
    let storage = StorageManager::<Objects>::new(
        StorageBackends {
            primary: data.clone(),
            state: Arc::new(Recovery::new(identity.clone())),
        },
        settings.clone(),
    )
    .unwrap();
    let runtime = Runtime::<Bank>::builder(Ledger::default(), ())
        .register(catalog())
        .runtime_with_storage(&storage)
        .await
        .unwrap();

    // Store Schema, Objects, Generated Records And Balanced Three-Phase Samples.
    let schema = waveforms::schema();
    storage
        .write_schema(&schema, &waveforms::definition())
        .await
        .unwrap();
    let object = Experiment {
        id: "experiment".into(),
        label: "Three Phase".into(),
    };
    let object_receipt = storage.write_object(&object).await.unwrap();
    storage
        .write_object(&Experiment {
            id: object.id.clone(),
            label: "Final".into(),
        })
        .await
        .unwrap();
    let samples = waveforms::samples(4);
    let inputs = waveforms::inputs(&samples);
    let records: Vec<_> = inputs
        .iter()
        .take(9)
        .map(|input| input.record.borrowed())
        .collect();
    storage.write_records(&records).await.unwrap();
    for inputs in inputs.chunks(192) {
        let batch: Vec<_> = inputs.iter().map(StreamInput::borrowed).collect();
        storage.write_stream(&batch).await.unwrap();
    }

    // Empty, Small And Multi-Batch Files Use Deterministic Binary Contents.
    let mut files = Vec::new();
    for length in [0, 17, settings.response.batch_bytes.get() * 2 + 13] {
        let bytes = waveforms::binary(length);
        let metadata = FileMetadata {
            id: Identifier::new(format!("binary-{length}")).unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: Some(length as u64),
            expected_sha256: None,
        };
        let reference = storage.write_file(metadata, &bytes).await.unwrap();
        files.push((reference, bytes));
    }
    let durable = storage.flush().await.unwrap();
    assert_eq!(storage.cursor_saved(), durable);

    assert_eq!(
        data.objects.lock().unwrap().len(),
        2,
        "unreleased lifecycle retains every revision"
    );
    // Round Trip Typed Objects And Finished File Bytes.
    let found = storage
        .read_object::<Experiment>(&object.id, ObjectVersion::Exact(object_receipt.version))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.value, object);
    for (reference, expected) in &files {
        let whole = read_range(
            &storage,
            reference,
            ByteRange {
                offset: 0,
                limit: None,
            },
        )
        .await;
        assert_eq!(&whole, expected);

        // Finished References Support Checked Half-Open Byte Ranges.
        let offset = reference.length.min(3);
        let selected = read_range(
            &storage,
            reference,
            ByteRange {
                offset,
                limit: Some(7),
            },
        )
        .await;
        assert_eq!(
            selected,
            expected[offset as usize..expected.len().min(offset as usize + 7)]
        );
        let end = read_range(
            &storage,
            reference,
            ByteRange {
                offset: reference.length,
                limit: None,
            },
        )
        .await;
        assert!(end.is_empty());
        let beyond = storage
            .read_file(ReadFile {
                file: reference.clone(),
                range: ByteRange {
                    offset: reference.length + 1,
                    limit: None,
                },
            })
            .await;
        assert!(beyond.is_err());
    }

    // Persisted Stream Values Independently Match The Generator And Analytical RMS.
    let mut reader = storage
        .query(Query {
            at: durable.clone(),
            source: Source::Stream(StreamSelection {
                schema: schema.clone(),
                scope: None,
                item: None,
                stream: None,
                source_start: None,
                source_end: None,
            }),
            transforms: vec![Transform::Project(Project {
                fields: vec![
                    Projection {
                        field: "payload.phase".into(),
                        name: "phase".into(),
                    },
                    Projection {
                        field: "payload.ordinal".into(),
                        name: "ordinal".into(),
                    },
                    Projection {
                        field: "payload.value".into(),
                        name: "value".into(),
                    },
                ],
            })],
            slice: RowSlice::default(),
        })
        .await
        .unwrap();
    let mut received = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        let QueryOutput::Batch(batch) = output else {
            continue;
        };
        let batch = batch.as_record_batch();
        let phase = column::<arrow_array::UInt32Array>(batch, "phase");
        let ordinal = column::<arrow_array::UInt64Array>(batch, "ordinal");
        let value = column::<arrow_array::Float64Array>(batch, "value");
        for row in 0..batch.num_rows() {
            received.push(Sample {
                phase: phase.value(row),
                ordinal: ordinal.value(row),
                value: value.value(row),
            });
        }
    }
    assert_eq!(received, samples);
    for phase in 0..3 {
        let values: Vec<_> = received
            .iter()
            .filter(|sample| sample.phase == phase)
            .map(|sample| sample.value)
            .collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let rms =
            (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt();
        assert!(mean.abs() < 1e-12);
        assert!((rms - waveforms::AMPLITUDE / 2.0_f64.sqrt()).abs() < 1e-12);
    }
    drop(reader);

    // Query One Half-Open Period And Aggregate Every Selected Sample.
    let selection = StreamSelection {
        schema: schema.clone(),
        scope: Some(Identifier::new("experiment").unwrap()),
        item: None,
        stream: None,
        source_start: Some(UNIX_EPOCH + Duration::from_millis(64)),
        source_end: Some(UNIX_EPOCH + Duration::from_millis(128)),
    };
    let mut reader = storage
        .query(Query {
            at: durable.clone(),
            source: Source::Stream(selection.clone()),
            transforms: vec![Transform::Aggregate(Aggregate {
                groups: vec![Projection {
                    field: "payload.phase".into(),
                    name: "phase".into(),
                }],
                measures: vec![Measure {
                    field: "payload.value".into(),
                    statistic: harmos::Aggregate::Rms,
                    name: "rms".into(),
                }],
            })],
            slice: RowSlice {
                offset: 0,
                limit: None,
            },
        })
        .await
        .unwrap();
    assert!(matches!(
        reader.next().await.unwrap(),
        Some(QueryOutput::Schema(_))
    ));
    let mut phases = 0;
    while let Some(output) = reader.next().await.unwrap() {
        let QueryOutput::Batch(batch) = output else {
            panic!("schema must be emitted once");
        };
        let values = batch
            .as_record_batch()
            .column_by_name("rms")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::Float64Array>()
            .unwrap();
        for value in values.values() {
            assert!((value - waveforms::AMPLITUDE / 2.0_f64.sqrt()).abs() < 1e-12);
            phases += 1;
        }
    }
    assert_eq!(phases, 3);
    assert_eq!(reader.completion().unwrap().at, durable);
    drop(reader);

    // Histograms Count Full Inputs; Display Downsampling Has A Separate Plan.
    let mut histogram = storage
        .query(Query {
            at: durable.clone(),
            source: Source::Stream(selection.clone()),
            transforms: vec![Transform::Histogram(Histogram {
                field: "payload.value".into(),
                edges: vec![-12.0, -6.0, 0.0, 6.0, 12.0],
            })],
            slice: RowSlice {
                offset: 0,
                limit: None,
            },
        })
        .await
        .unwrap();
    let mut counts_actual = Vec::new();
    let mut counted = 0;
    while let Some(output) = histogram.next().await.unwrap() {
        if let QueryOutput::Batch(batch) = output {
            let counts = batch
                .as_record_batch()
                .column_by_name("count")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::UInt64Array>()
                .unwrap();
            counted += counts.values().iter().sum::<u64>();
            counts_actual.extend_from_slice(counts.values());
        }
    }
    assert_eq!(counted, (waveforms::SAMPLES_PER_PERIOD * 3) as u64);
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
        samples
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
    assert_eq!(counts_actual, expected);
    drop(histogram);
    let mut display = storage
        .query(Query {
            at: durable.clone(),
            source: Source::Stream(selection),
            transforms: vec![Transform::Downsample(Downsample {
                max_rows: 32,
                algorithm: Algorithm::Extrema,
                axis: None,
                value: Some("payload.value".into()),
            })],
            slice: RowSlice {
                offset: 0,
                limit: None,
            },
        })
        .await
        .unwrap();
    let mut displayed = 0;
    while let Some(output) = display.next().await.unwrap() {
        if let QueryOutput::Batch(batch) = output {
            displayed += batch.as_record_batch().num_rows();
        }
    }
    assert!(displayed > 0 && displayed <= 32);
    assert_eq!(display.completion().unwrap().rows, displayed as u64);
    drop(display);
    let encoded = Objects::encode(&object, NonZeroUsize::new(16 * 1024).unwrap()).unwrap();
    storage
        .release_object_history(&encoded.schema, &encoded.item, &durable)
        .await
        .unwrap();
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
    drop(storage);
    drop(data);

    // Reopen Rebuilds Indexes From The Journal And Preserves The Durable Boundary.
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(directory.path().join("journal.jsonl"))
        .unwrap()
        .write_all(b"{uncommitted")
        .unwrap();
    let reopened = Arc::new(Data::<Objects>::journaled(directory.path(), identity).unwrap());
    assert_eq!(reopened.cursor_saved().await.unwrap(), durable);
    assert_eq!(reopened.rows.lock().unwrap().len(), samples.len());
    assert_eq!(reopened.records.lock().unwrap().len(), 9);
    assert_eq!(reopened.object_releases.lock().unwrap().len(), 1);
    assert_eq!(reopened.object_releases.lock().unwrap()[0].through, durable);
    for (reference, bytes) in files {
        assert_eq!(
            reopened.blobs.lock().unwrap()[reference.metadata.id.as_str()].1,
            bytes
        );
    }
    assert!(directory.path().join("journal.jsonl").is_file());
    assert_eq!(
        std::fs::read_dir(directory.path().join("staging"))
            .unwrap()
            .count(),
        0
    );
}
