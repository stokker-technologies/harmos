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
use std::sync::Arc;

// Third Party Imports
use arrow_array::{ArrayRef, Float64Array, RecordBatch, UInt32Array};
use arrow_schema::{DataType, Field, Schema};
use criterion::{BenchmarkId, Criterion, Throughput};
use harmos_runtime::storage::query::transform::TransformExecution;
use harmos_runtime::storage::query::transform::{
    Transform,
    aggregate::{Aggregate, Measure},
    downsample::{Algorithm, Downsample},
    histogram::Histogram,
    project::Projection,
};

// Local Imports
use super::waveforms;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(criterion: &mut Criterion, _: &tokio::runtime::Runtime) {
    let mut group = criterion.benchmark_group("storage/transforms");
    group.sample_size(10);
    group.sampling_mode(criterion::SamplingMode::Flat);
    for periods in [1, 6] {
        let samples = waveforms::samples(periods);
        let schema =
            Arc::new(Schema::new(vec![Field::new("phase", DataType::UInt32, false), Field::new("value", DataType::Float64, false)]));
        let columns: Vec<ArrayRef> = vec![
            Arc::new(UInt32Array::from(samples.iter().map(|sample| sample.phase).collect::<Vec<_>>())),
            Arc::new(Float64Array::from(samples.iter().map(|sample| sample.value).collect::<Vec<_>>())),
        ];
        let input = RecordBatch::try_new(schema, columns).unwrap();
        let transforms = [
            (
                "rms",
                Transform::Aggregate(Aggregate {
                    groups: vec![Projection {
                        field: "phase".into(),
                        name: "phase".into(),
                    }],
                    measures: vec![Measure {
                        field: "value".into(),
                        statistic: harmos_runtime::Aggregate::Rms,
                        name: "rms".into(),
                    }],
                }),
            ),
            (
                "histogram",
                Transform::Histogram(Histogram {
                    field: "value".into(),
                    edges: vec![-12.0, -6.0, 0.0, 6.0, 12.0],
                }),
            ),
            (
                "downsample",
                Transform::Downsample(Downsample {
                    max_rows: 128,
                    algorithm: Algorithm::Extrema,
                    axis: None,
                    value: Some("value".into()),
                }),
            ),
        ];
        group.throughput(Throughput::Elements(samples.len() as u64));
        for (name, transform) in transforms {
            // Independent Correctness Checks Stay Outside The Measured Kernel.
            let result = transform.apply(&input, &mut || Ok(())).unwrap();
            if name == "rms" {
                let values = result.column_by_name("rms").unwrap().as_any().downcast_ref::<Float64Array>().unwrap();
                assert_eq!(values.len(), 3);
                for value in values.values() {
                    assert!((value - waveforms::AMPLITUDE / 2.0_f64.sqrt()).abs() < 1e-12);
                }
            } else if name == "histogram" {
                let counts = result.column_by_name("count").unwrap().as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
                assert_eq!(counts.values().iter().sum::<u64>(), samples.len() as u64);
            } else {
                assert!(result.num_rows() <= 128);
            }
            group.bench_function(BenchmarkId::new(name, samples.len()), |bencher| {
                bencher.iter_custom(|iterations| {
                    let probe = super::metrics::Probe::start_process();
                    let started = std::time::Instant::now();
                    for _ in 0..iterations {
                        std::hint::black_box(transform.apply(std::hint::black_box(&input), &mut || Ok(())).unwrap());
                    }
                    let elapsed = started.elapsed();
                    probe.finish().report(&format!("transforms/{name}/kernel"), samples.len(), 1, None);
                    elapsed
                })
            });
        }
    }
    group.finish();
}
/* === Private Functions === */

/* === Tests === */
