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
use std::time::{Duration as Elapsed, UNIX_EPOCH};

// Third Party Imports

// Local Imports
use super::*;
use harmos::Message;
use harmos::storage::frame::*;
use harmos::storage::reduce::{Agg, ExactNumber, ScalarKind, ScalarValue};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn value() -> ScalarField {
    ScalarField {
        path: ScalarPath::Payload(vec![Identifier::new("value").unwrap()]),
        kind: ScalarKind::Unsigned,
    }
}
fn selection() -> ReadSelection {
    let mut selection = ReadSelection::new(ReadKind::Records);
    selection.schema = Some(schema());
    selection
}
fn request() -> FrameQuery {
    let mut frame = Frame::new(
        NonZeroUsize::new(100).unwrap(),
        NonZeroUsize::new(16).unwrap(),
    );
    let scan = frame.scan(selection(), vec![value()]);
    frame.group_by(
        GroupBy::new(scan)
            .agg(Expr::Col(0), Agg::Mean)
            .agg(Expr::Col(0), Agg::Len),
    );
    FrameQuery {
        frame,
        options: QueryOptions::default(),
    }
}
async fn seed(fixture: &Fixture, values: &[u64]) {
    let field = harmos::schema::Field {
        name: "value".into(),
        tag: 1,
        kind: harmos::schema::Kind::U64,
        cardinality: harmos::schema::Cardinality::Required,
    };
    fixture
        .storage
        .write_schema(&schema(), &[field])
        .await
        .unwrap();
    let scope = Identifier::new("line").unwrap();
    for (index, value) in values.iter().enumerate() {
        // Keep Source Time Independent Of Exact Numeric Payloads
        let source_seconds = (index as u64 + 1).pow(2);
        let payload = value.encode_to_vec();
        let item = Identifier::new(format!("value{index}")).unwrap();
        fixture
            .storage
            .write_records(&[RecordWrite {
                scope: Some(&scope),
                source_time: Some(UNIX_EPOCH + Elapsed::from_secs(source_seconds)),
                ..record(&schema(), &item, &payload)
            }])
            .await
            .unwrap();
    }
    fixture.storage.flush().await.unwrap();
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("backend_uses_cut", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, &[9_007_199_254_740_993, 9_007_199_254_740_995]).await;
                    let mut reader = f.storage.query(request()).await.unwrap();
                    let saved = reader.view().saved().unwrap();
                    let extra = 0u64.encode_to_vec();
                    f.storage
                        .write_records(&[record(
                            &schema(),
                            &Identifier::new("later").unwrap(),
                            &extra,
                        )])
                        .await
                        .unwrap();
                    f.storage.flush().await.unwrap();
                    assert!(f.storage.cursor_saved().sequence > saved.sequence);

                    let first = reader.read().await.unwrap();
                    for _ in 0..64 {
                        let binding = reader.read().await.unwrap();
                        assert!(std::ptr::eq(&*first, &*binding));
                    }
                    assert_eq!(
                        first.outputs[0].groups[0].values,
                        vec![
                            ScalarValue::Unsigned(9_007_199_254_740_994),
                            ScalarValue::Unsigned(2)
                        ]
                    );
                    assert_eq!(f.data.control.frame_pages.load(Ordering::SeqCst), 1);
                    assert_eq!(f.data.control.frame_decodes.load(Ordering::SeqCst), 2);

                    let projected = reader
                        .view()
                        .project_shared(&first, NonZeroUsize::new(1024).unwrap(), |batch, _| {
                            Ok(batch.outputs[0].groups[0].values.clone())
                        })
                        .unwrap();
                    assert_eq!(projected[0], ScalarValue::Unsigned(9_007_199_254_740_994));
                    drop(projected);
                    drop(first);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("cancellation_preserves_view", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, &[1, 3]).await;
                    let mut reader = f.storage.query(request()).await.unwrap();
                    let saved = reader.view().saved().unwrap();
                    f.data.control.block_read.store(true, Ordering::SeqCst);
                    {
                        let read = reader.read();
                        tokio::pin!(read);
                        tokio::select! {
                            result = &mut read => panic!("unexpected result: {}", result.is_ok()),
                            _ = f.data.control.read_entered.notified() => {}
                        }
                    }
                    f.data.control.block_read.store(false, Ordering::SeqCst);
                    assert_eq!(reader.view().saved().unwrap(), saved);
                    assert_eq!(
                        reader.read().await.unwrap().outputs[0].groups[0].values[1],
                        ScalarValue::Unsigned(2)
                    );
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("queries_fail_snapshot", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    let before = f.data.control.query_views.load(Ordering::SeqCst);
                    let mut unsupported = request();
                    unsupported.frame.scans[0].selection.kind = ReadKind::Files;
                    assert!(f.storage.query(unsupported).await.is_err());

                    let mut ignored_cut = request();
                    ignored_cut.options.after = 1;
                    assert!(f.storage.query(ignored_cut).await.is_err());

                    let mut invalid = request();
                    invalid.frame.group_bys[0].aggs[0].0 = Expr::Col(9);
                    assert!(f.storage.query(invalid).await.is_err());

                    let mut non_numeric = request();
                    non_numeric.frame.scans[0].columns[0] = ScalarField {
                        path: ScalarPath::Item,
                        kind: ScalarKind::Text,
                    };
                    assert!(f.storage.query(non_numeric).await.is_err());

                    let mut wrong_predicate = request();
                    wrong_predicate.frame.group_bys[0].filter.push(Predicate {
                        expr: Expr::Col(0),
                        compare: Compare::Eq,
                        value: ScalarValue::Text("wrong type".into()),
                    });
                    assert!(f.storage.query(wrong_predicate).await.is_err());

                    // A Keyed Order The Backend Cannot Serve Refuses The Whole Frame Up Front.
                    f.data
                        .control
                        .frame_capture_only
                        .store(true, Ordering::SeqCst);
                    let mut keyed = request();
                    let shift = keyed.frame.shift(0, vec![0], 0);
                    keyed.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::SourceTime,
                        kind: ScalarKind::Datetime,
                    });
                    keyed.frame.as_ofs[shift].on = (1, 1);
                    keyed.frame.group_bys[0]
                        .aggs
                        .push((Expr::diff(shift), Agg::Mean));
                    assert!(f.storage.query(keyed).await.is_err());
                    assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("explained_peak_beyond", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, &[1, 2]).await;
                    let mut median = request();
                    median.frame.max_rows = NonZeroUsize::new(1 << 20).unwrap();
                    median.frame.group_bys[0].aggs[0].1 = Agg::Median;
                    let explain = median.frame.explain(config().query_page).unwrap();
                    assert!(explain.median_scratch_bytes > f.execution.memory_bytes.get());
                    assert!(f.storage.query(median).await.is_err());
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("aggregates_share_pass", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, &[1, 4, 9, 16]).await;
                    let mut query = request();
                    query.options.page = Some(ReadLimits {
                        rows: NonZeroUsize::new(2).unwrap(),
                        bytes: NonZeroUsize::new(4096).unwrap(),
                    });
                    query.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::Scope,
                        kind: ScalarKind::Text,
                    });
                    query.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::SourceTime,
                        kind: ScalarKind::Datetime,
                    });
                    let shift = query.frame.join_asof(AsOf {
                        left: 0,
                        right: 0,
                        by: vec![(1, 1)],
                        on: (2, 2),
                        strategy: AsOfStrategy::Previous,
                        right_filter: vec![],
                    });
                    query.frame.group_by(
                        GroupBy::new(0)
                            .agg(Expr::diff(shift), Agg::Max)
                            .agg(Expr::diff(shift), Agg::Count),
                    );
                    query
                        .frame
                        .group_by(GroupBy::new(0).agg(Expr::Col(0), Agg::Median));
                    let mut reader = f.storage.query(query).await.unwrap();
                    let collected = reader.read().await.unwrap();

                    // Elapsed Nanoseconds Between Consecutive Rows Of One Scope: 3s, 5s, 7s.
                    assert_eq!(
                        collected.outputs[1].groups[0].values,
                        vec![
                            ScalarValue::Unsigned(7_000_000_000),
                            ScalarValue::Unsigned(3)
                        ]
                    );
                    assert_eq!(
                        collected.outputs[2].groups[0].values,
                        vec![ScalarValue::Exact(ExactNumber::new(
                            false,
                            13,
                            std::num::NonZeroU128::new(2).unwrap()
                        ))]
                    );
                    // Capture Pass Plus Keyed Pass, Each Decoding The Four Rows Once.
                    assert_eq!(f.data.control.frame_decodes.load(Ordering::SeqCst), 8);
                    drop(collected);
                    drop(reader);
                    f.finish().await;
                });
        });
    }
    {
        scenario_runtime::check("backward_join_merges", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let f = Fixture::new(config()).await;
                    seed(&f, &[1, 4, 9, 16]).await;
                    let mut query = request();
                    query.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::Scope,
                        kind: ScalarKind::Text,
                    });
                    query.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::SourceTime,
                        kind: ScalarKind::Datetime,
                    });
                    let join = query.frame.join_asof(AsOf {
                        left: 0,
                        right: 0,
                        by: vec![(1, 1)],
                        on: (2, 2),
                        strategy: AsOfStrategy::Backward { present: 0 },
                        right_filter: vec![Predicate {
                            expr: Expr::Col(0),
                            compare: Compare::Lt,
                            value: ScalarValue::Unsigned(9),
                        }],
                    });
                    query.frame.group_bys[0] = GroupBy::new(0)
                        .agg(Expr::AsOf { join, col: 0 }, Agg::Last)
                        .agg(Expr::AsOf { join, col: 0 }, Agg::Count);
                    let mut reader = f.storage.query(query).await.unwrap();

                    // Rows At 9s And 16s Fall Back To The Latest Admitted Row, 4.
                    assert_eq!(
                        reader.read().await.unwrap().outputs[0].groups[0].values,
                        vec![ScalarValue::Unsigned(4), ScalarValue::Unsigned(4)]
                    );
                    assert_eq!(f.data.control.frame_decodes.load(Ordering::SeqCst), 8);
                    drop(reader);

                    // Without Any Key The Join Refuses: Ownership Is Never Inferred.
                    let mut keyless = request();
                    keyless.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::SourceTime,
                        kind: ScalarKind::Datetime,
                    });
                    let shift = keyless.frame.shift(0, vec![], 1);
                    keyless.frame.as_ofs[shift].by.clear();
                    assert!(f.storage.query(keyless).await.is_err());
                    f.finish().await;
                });
        });
    }
}
