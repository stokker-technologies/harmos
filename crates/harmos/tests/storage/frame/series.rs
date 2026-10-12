/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;
use harmos::storage::frame::{ScalarField, ScalarPath};
use harmos::storage::reduce::{Agg, ExactNumber, ScalarKind, ScalarValue};
use harmos::storage::series::SeriesPoint;
use harmos::{Message, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn seed(f: &Fixture, count: u64) {
    let field = harmos::schema::Field {
        name: "value".into(),
        tag: 1,
        kind: harmos::schema::Kind::U64,
        cardinality: harmos::schema::Cardinality::Required,
    };
    f.storage.write_schema(&schema(), &[field]).await.unwrap();
    for index in 0..count {
        let payload = index.encode_to_vec();
        f.storage
            .write_stream(&[StreamWrite {
                record: record(
                    &schema(),
                    &Identifier::new(format!("sample{index}")).unwrap(),
                    &payload,
                ),
                stream: &Identifier::new("samples").unwrap(),
                sequence: index % 2,
            }])
            .await
            .unwrap();
    }
    f.storage.flush().await.unwrap();
}
fn series(samples: usize) -> harmos::storage::series::SeriesQuery<u64> {
    series_projected(samples, |row, _| {
        Ok(SeriesPoint {
            axis: ScalarValue::Unsigned(row.record.position.sequence),
            value: ScalarValue::Unsigned(row.record.payload),
        })
    })
}
fn series_projected(
    samples: usize,
    project: impl Fn(&StoredRow<u64>, NonZeroUsize) -> Result<SeriesPoint> + Send + Sync + 'static,
) -> harmos::storage::series::SeriesQuery<u64> {
    let mut query = StreamQuery::new(schema(), |bytes, _| {
        u64::decode(bytes).map_err(|_| Error::InvalidInput {
            field: "fixture",
            reason: "invalid sample",
        })
    });
    query.options.page = Some(ReadLimits {
        rows: NonZeroUsize::new(2).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    });
    query
        .series(
            ScalarField {
                path: ScalarPath::CaptureOrder,
                kind: ScalarKind::Unsigned,
            },
            ScalarField {
                path: ScalarPath::Payload(vec![Identifier::new("value").unwrap()]),
                kind: ScalarKind::Unsigned,
            },
            NonZeroUsize::new(samples).unwrap(),
            project,
        )
        .unwrap()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("series_opens_lazily", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, 4).await;
                    let series = f.storage.query(series(16)).await.unwrap();
                    assert_eq!(f.data.control.frame_decodes.load(Ordering::SeqCst), 0);
                    let mut aggregate = series.aggregate(vec![Agg::Mean, Agg::Median]).unwrap();
                    let result = aggregate.read().await.unwrap();
                    assert_eq!(
                        result.outputs[0].groups[0].values,
                        vec![
                            ScalarValue::Exact(ExactNumber::new(
                                false,
                                3,
                                std::num::NonZeroU128::new(2).unwrap()
                            ));
                            2
                        ]
                    );
                    assert_eq!(
                        series.view().unwrap().saved().unwrap(),
                        aggregate.view().saved().unwrap()
                    );
                    drop(result);
                    drop(aggregate);
                    drop(series);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("downsample_preserves_axis", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, 8).await;
                    let mut series = f.storage.query(series(16)).await.unwrap();
                    let reduced = series
                        .transform()
                        .downsample(
                            NonZeroUsize::new(2).unwrap(),
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(reduced.points.len(), 2);
                    assert_eq!(reduced.points[0].axis, ScalarValue::Unsigned(2));
                    assert_eq!(reduced.points[1].axis, ScalarValue::Unsigned(6));
                    assert_eq!(reduced.points[1].value, ScalarValue::Unsigned(4));
                    let mut aggregate = series.aggregate(vec![Agg::Len]).unwrap();
                    assert_eq!(
                        aggregate.read().await.unwrap().outputs[0].groups[0].values,
                        vec![ScalarValue::Unsigned(8)]
                    );
                    drop(aggregate);
                    drop(reduced);
                    drop(series);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("refusal_retains_pages", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, 8).await;
                    let mut series = f.storage.query(series(16)).await.unwrap();
                    assert!(
                        series
                            .transform()
                            .downsample(NonZeroUsize::new(2).unwrap(), NonZeroUsize::MIN)
                            .await
                            .is_err()
                    );
                    let first = series
                        .transform()
                        .downsample(
                            NonZeroUsize::new(2).unwrap(),
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap();
                    let second = series
                        .transform()
                        .downsample(
                            NonZeroUsize::new(2).unwrap(),
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(*first, *second);
                    drop(first);
                    drop(second);
                    drop(series);
                    f.finish().await;
                });
        });
    }
    #[cfg(feature = "signal-fft")]
    {
        scenario_runtime::check("axis_refuses_unsupported", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, 8).await;
                    let mut series = f.storage.query(series(16)).await.unwrap();
                    let spectrum = series
                        .transform()
                        .fft(
                            8.0,
                            harmos::signal::Taper::Rectangular,
                            NonZeroUsize::new(4096).unwrap(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(spectrum.frequency, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
                    assert_eq!(spectrum.real.len(), 5);
                    assert!((spectrum.real[0] - 3.5).abs() < 1e-12);
                    drop(spectrum);
                    drop(series);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("limit_never_returns", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, 4).await;
                    let mut series = f.storage.query(series(3)).await.unwrap();
                    assert!(
                        series
                            .transform()
                            .downsample(NonZeroUsize::MIN, NonZeroUsize::new(4096).unwrap())
                            .await
                            .is_err()
                    );
                    drop(series);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("transform_resumes_pages", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor.enable_all().build().expect("scenario runtime is valid").block_on(async {

    let f = Fixture::new(config()).await;
    seed(&f, 8).await;
    let data = f.data.clone();
    let query = series_projected(16, move |row, _| {
        if row.record.payload == 1 {
            data.control.block_read.store(true, Ordering::SeqCst);
        }
        Ok(SeriesPoint {
            axis: ScalarValue::Unsigned(row.record.position.sequence),
            value: ScalarValue::Unsigned(row.record.payload),
        })
    });
    let mut series = f.storage.query(query).await.unwrap();
    let saved = series.view().unwrap().saved().unwrap();
    {
        let transform = series.transform().downsample(
            NonZeroUsize::new(8).unwrap(),
            NonZeroUsize::new(4096).unwrap(),
        );
        tokio::pin!(transform);
        tokio::select! {
            result = &mut transform => panic!("unexpected result: {}", result.is_ok()),
            _ = f.data.control.read_entered.notified() => {}
        }
    }
    f.data.control.block_read.store(false, Ordering::SeqCst);
    let result = series
        .transform()
        .downsample(
            NonZeroUsize::new(8).unwrap(),
            NonZeroUsize::new(4096).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        result
            .points
            .iter()
            .map(|point| point.value.clone())
            .collect::<Vec<_>>(),
        (0..8).map(ScalarValue::Unsigned).collect::<Vec<_>>()
    );
    assert_eq!(series.view().unwrap().saved().unwrap(), saved);
    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), 1);
    drop(result);
    drop(series);
    f.finish().await;

});
        });
    }
    #[cfg(feature = "signal-fft")]
    {
        scenario_runtime::check("fft_refuses_non", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, 6).await;
                    let irregular = f
                        .storage
                        .query(series_projected(16, |row, _| {
                            Ok(SeriesPoint {
                                axis: ScalarValue::Unsigned(
                                    row.record.payload * row.record.payload,
                                ),
                                value: ScalarValue::Unsigned(row.record.payload),
                            })
                        }))
                        .await
                        .unwrap();
                    let mut non_power = f.storage.query(series(16)).await.unwrap();
                    assert!(
                        non_power
                            .transform()
                            .fft(
                                6.0,
                                harmos::signal::Taper::Rectangular,
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await
                            .is_err()
                    );
                    drop(non_power);
                    // An Exact Four-Sample Cut Tests Axis Admission Independently Of FFT Shape.
                    let mut query = series_projected(16, |row, _| {
                        Ok(SeriesPoint {
                            axis: ScalarValue::Unsigned(row.record.payload * row.record.payload),
                            value: ScalarValue::Unsigned(row.record.payload),
                        })
                    });
                    query.options_mut().through = Some(5);
                    let mut four = f
                        .storage
                        .query_in(irregular.view().unwrap(), query)
                        .unwrap();
                    assert!(
                        four.transform()
                            .fft(
                                4.0,
                                harmos::signal::Taper::Rectangular,
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await
                            .is_err()
                    );
                    // A Refused FFT Retains The Original Points For Another Transform.
                    assert_eq!(
                        four.transform()
                            .downsample(
                                NonZeroUsize::new(4).unwrap(),
                                NonZeroUsize::new(4096).unwrap()
                            )
                            .await
                            .unwrap()
                            .points
                            .len(),
                        4
                    );
                    drop(four);
                    drop(irregular);
                    f.finish().await;
                });
        });
    }
}
