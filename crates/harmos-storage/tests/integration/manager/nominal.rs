/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::nominal_fixture::*;

/* === Tests === */

#[tokio::test]
async fn stored_waveform_resumes_summarizes_and_downsamples() {
    // Store The Waveform Once
    let n = Nominal::new().await;

    // Persisted Stream Values Match The Generator And Analytical RMS.
    let projection = project(&[("payload.phase", "phase"), ("payload.ordinal", "ordinal"), ("payload.value", "value")]);
    let mut received = Vec::new();
    let read = batches(&n.storage, n.query(n.all(0), vec![projection])).await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    assert_eq!(read.completed_rows, read.batches.iter().map(|batch| batch.num_rows() as u64).sum::<u64>());
    for batch in read.batches {
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
        let values: Vec<_> = received.iter().filter(|sample| sample.phase == phase).map(|sample| sample.value).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let measured = (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt();
        assert!(mean.abs() < 1e-12);
        assert!((measured - rms()).abs() < 1e-12);
    }

    // Resuming After A Stored Position Yields Exactly The Later Rows, In Order.
    let positioned = |after| n.query(n.all(after), vec![project(&[("_storage.position", "position"), ("payload.ordinal", "ordinal")])]);
    let read = batches(&n.storage, positioned(0)).await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    assert_eq!(read.completed_rows, read.batches.iter().map(|batch| batch.num_rows() as u64).sum::<u64>());
    let all = positions(&read);
    assert_eq!(all.len(), n.samples.len());
    let middle = all[all.len() / 2].0;
    let read = batches(&n.storage, positioned(middle)).await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    assert_eq!(read.completed_rows, read.batches.iter().map(|batch| batch.num_rows() as u64).sum::<u64>());
    let resumed = positions(&read);
    assert_eq!(resumed, all[all.len() / 2 + 1..]);
    assert!(resumed.windows(2).all(|pair| pair[0].0 < pair[1].0));

    // One Half-Open Period Aggregates Every Selected Sample Per Phase.
    let aggregate = Transform::Aggregate(Aggregate {
        groups: vec![Projection {
            field: "payload.phase".into(),
            name: "phase".into(),
        }],
        measures: vec![Measure {
            field: "payload.value".into(),
            statistic: harmos_domain::Aggregate::Rms,
            name: "rms".into(),
        }],
    });
    let mut phases = 0;
    let read = batches(&n.storage, n.query(n.period(), vec![aggregate])).await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    assert_eq!(read.completed_rows, read.batches.iter().map(|batch| batch.num_rows() as u64).sum::<u64>());
    for batch in read.batches {
        for value in column::<arrow_array::Float64Array>(&batch, "rms").values() {
            assert!((value - rms()).abs() < 1e-12);
            phases += 1;
        }
    }
    assert_eq!(phases, 3);

    // Histograms Count Every Selected Input Into Closed-Ended Bins.
    let histogram = Transform::Histogram(Histogram {
        field: "payload.value".into(),
        edges: vec![-12.0, -6.0, 0.0, 6.0, 12.0],
    });
    let mut counts = Vec::new();
    let read = batches(&n.storage, n.query(n.period(), vec![histogram])).await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    assert_eq!(read.completed_rows, read.batches.iter().map(|batch| batch.num_rows() as u64).sum::<u64>());
    for batch in read.batches {
        counts.extend_from_slice(column::<arrow_array::UInt64Array>(&batch, "count").values());
    }
    assert_eq!(counts.iter().sum::<u64>(), (waveforms::SAMPLES_PER_PERIOD * 3) as u64);
    let expected: Vec<u64> = [(-f64::INFINITY, -12.0), (-12.0, -6.0), (-6.0, 0.0), (0.0, 6.0), (6.0, 12.0), (12.0, f64::INFINITY)]
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

    // Display Downsampling Delivers A Bounded, Non-Empty Plan.
    let downsample = Transform::Downsample(Downsample {
        max_rows: 32,
        algorithm: Algorithm::Extrema,
        axis: None,
        value: Some("payload.value".into()),
    });
    let read = batches(&n.storage, n.query(n.period(), vec![downsample])).await;
    assert!(read.schema_first);
    assert_eq!(read.schemas, 1);
    assert_eq!(read.completed_rows, read.batches.iter().map(|batch| batch.num_rows() as u64).sum::<u64>());
    let displayed: usize = read.batches.iter().map(|batch| batch.num_rows()).sum();
    assert!(displayed > 0 && displayed <= 32);

    // Stop
    let storage = n.storage.clone();
    n.finish().await;
    assert!(storage.status().closed);
    assert!(!storage.status().running);
}
