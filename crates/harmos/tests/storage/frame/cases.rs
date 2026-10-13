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
#[harmos::message]
struct Ownership {
    session: String,
    cycle: Option<String>,
}
#[harmos::message]
struct Evidence {
    value: u64,
    owner: Option<Ownership>,
}

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
    let field = harmos::schema::graph::Field {
        name: "value".into(),
        tag: 1,
        kind: harmos::schema::graph::Kind::U64,
        cardinality: harmos::schema::Cardinality::Required,
    };
    fixture
        .storage
        .write_schema(
            &schema(),
            &harmos::schema::graph::Schema {
                root: 0,
                definitions: vec![vec![field]],
            },
        )
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
        scenario_runtime::check("raw_record_filters_keep_paging_and_scope", || {
            let mut executor = tokio::runtime::Builder::new_current_thread();
            executor.enable_all().build().unwrap().block_on(async {
                let f = Fixture::new(config()).await;
                f.storage
                    .write_schema(
                        &schema(),
                        &harmos::schema::graph::Schema::from_declaration(
                            <Evidence as harmos::schema::Described>::DEFINITION,
                            &harmos::config::SchemaSettings::default(),
                        )
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                let epoch = Identifier::new("process").unwrap();
                let other = Identifier::new("foreign-process").unwrap();
                for (index, (value, owner, process)) in [
                    (
                        2,
                        Some(Ownership {
                            session: "a".into(),
                            cycle: Some("one".into()),
                        }),
                        &epoch,
                    ),
                    (2, None, &epoch),
                    (
                        4,
                        Some(Ownership {
                            session: "a".into(),
                            cycle: Some("one".into()),
                        }),
                        &epoch,
                    ),
                    (
                        2,
                        Some(Ownership {
                            session: "b".into(),
                            cycle: Some("one".into()),
                        }),
                        &epoch,
                    ),
                    (
                        2,
                        Some(Ownership {
                            session: "a".into(),
                            cycle: Some("one".into()),
                        }),
                        &other,
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let payload = Evidence { value, owner }.encode_to_vec();
                    let item = Identifier::new(format!("raw{index}")).unwrap();
                    f.storage
                        .write_records(&[RecordWrite {
                            scope: Some(process),
                            ..record(&schema(), &item, &payload)
                        }])
                        .await
                        .unwrap();
                }
                f.storage.flush().await.unwrap();
                let filter = harmos::storage::RecordFilter {
                    columns: vec![
                        value(),
                        ScalarField {
                            path: ScalarPath::Payload(vec![
                                Identifier::new("owner").unwrap(),
                                Identifier::new("session").unwrap(),
                            ]),
                            kind: ScalarKind::Text,
                        },
                        ScalarField {
                            path: ScalarPath::Payload(vec![
                                Identifier::new("owner").unwrap(),
                                Identifier::new("cycle").unwrap(),
                            ]),
                            kind: ScalarKind::Text,
                        },
                    ],
                    clauses: vec![harmos::storage::RecordClause {
                        predicates: vec![
                            Predicate::In {
                                expr: Expr::Col(0),
                                values: vec![ScalarValue::Unsigned(2), ScalarValue::Unsigned(4)],
                            },
                            Predicate::Compare {
                                expr: Expr::Col(1),
                                compare: Compare::Eq,
                                value: ScalarValue::Text("a".into()),
                            },
                            Predicate::Exists {
                                expr: Expr::Col(2),
                                present: true,
                            },
                            Predicate::Compare {
                                expr: Expr::Col(2),
                                compare: Compare::Eq,
                                value: ScalarValue::Text("one".into()),
                            },
                        ],
                    }],
                };
                assert_eq!(
                    harmos::storage::RecordFilter::decode(&filter.encode_to_vec()).unwrap(),
                    filter
                );
                let make = |filter| {
                    let mut query = RecordQuery::new(schema(), decode_bytes);
                    query.filter = Some(filter);
                    query.scope = Some(epoch.clone());
                    query.options.page = Some(ReadLimits {
                        rows: NonZeroUsize::MIN,
                        bytes: NonZeroUsize::new(4096).unwrap(),
                    });
                    query
                };
                let mut reader = f.storage.query(make(filter.clone())).await.unwrap();
                let saved = reader.view().unwrap().saved().unwrap();
                let late = Evidence {
                    value: 2,
                    owner: Some(Ownership {
                        session: "a".into(),
                        cycle: Some("one".into()),
                    }),
                }
                .encode_to_vec();
                f.storage
                    .write_records(&[RecordWrite {
                        scope: Some(&epoch),
                        ..record(&schema(), &Identifier::new("late").unwrap(), &late)
                    }])
                    .await
                    .unwrap();
                f.storage.flush().await.unwrap();
                assert!(f.storage.cursor_saved().sequence > saved.sequence);
                let mut items = Vec::new();
                while let Some(page) = reader.next_page().await.unwrap() {
                    assert_eq!(page.len(), 1);
                    assert_eq!(page[0].scope.as_ref(), Some(&epoch));
                    items.push(page[0].item.to_string());
                }
                assert_eq!(items, ["raw0", "raw2"]);
                drop(reader);
                let mut descending = make(filter.clone());
                descending.options.order = ReadOrder::Descending;
                descending.options.through = Some(saved.sequence);
                let mut reader = f.storage.query(descending).await.unwrap();
                let mut items = Vec::new();
                while let Some(page) = reader.next_page().await.unwrap() {
                    items.push(page[0].item.to_string());
                }
                assert_eq!(items, ["raw2", "raw0"]);
                drop(reader);
                let mut union = filter.clone();
                union.columns.push(ScalarField {
                    path: ScalarPath::Item,
                    kind: ScalarKind::Text,
                });
                union.clauses.push(harmos::storage::RecordClause {
                    predicates: vec![Predicate::In {
                        expr: Expr::Col(3),
                        values: vec![
                            ScalarValue::Text("raw0".into()),
                            ScalarValue::Text("raw1".into()),
                            ScalarValue::Text("raw4".into()),
                        ],
                    }],
                });
                assert_eq!(
                    harmos::storage::RecordFilter::decode(&union.encode_to_vec()).unwrap(),
                    union,
                );
                let view = f.storage.view_with_limits(f.execution).await.unwrap();
                for (order, expected) in [
                    (ReadOrder::Ascending, ["raw0", "raw1", "raw2"]),
                    (ReadOrder::Descending, ["raw2", "raw1", "raw0"]),
                ] {
                    let mut selection =
                        harmos::storage::ReadSelection::new(harmos::storage::ReadKind::Records);
                    selection.schema = Some(schema());
                    selection.scope = Some(epoch.clone());
                    selection.through = Some(saved.sequence);
                    selection.order = order;
                    selection.record_filter = Some(union.clone());
                    let mut reader = view.read_records(selection).unwrap();
                    let mut items = Vec::new();
                    while let Some(page) = reader
                        .next_page(ReadLimits {
                            rows: NonZeroUsize::MIN,
                            bytes: NonZeroUsize::new(4096).unwrap(),
                        })
                        .await
                        .unwrap()
                    {
                        assert_eq!(page.len(), 1);
                        items.push(page[0].item.to_string());
                    }
                    assert_eq!(items, expected);
                }
                drop(view);
                let timed = QueryLimits {
                    timeout: Duration::from_millis(50),
                    ..f.execution
                };
                let timed_view = f.storage.view_with_limits(timed).await.unwrap();
                let mut timed_selection =
                    harmos::storage::ReadSelection::new(harmos::storage::ReadKind::Records);
                timed_selection.schema = Some(schema());
                timed_selection.through = Some(saved.sequence);
                timed_selection.record_filter = Some(union.clone());
                let mut timed_reader = timed_view.read_records(timed_selection).unwrap();
                let limits = ReadLimits {
                    rows: NonZeroUsize::MIN,
                    bytes: NonZeroUsize::new(4096).unwrap(),
                };
                f.data.control.block_read.store(true, Ordering::SeqCst);
                assert!(timed_reader.next_page(limits).await.is_err());
                f.data.control.block_read.store(false, Ordering::SeqCst);
                assert!(timed_reader.next_page(limits).await.is_err());
                drop(timed_reader);
                drop(timed_view);
                let mut invalid = union.clone();
                invalid.clauses.clear();
                assert!(invalid.check().is_err());
                invalid
                    .clauses
                    .push(harmos::storage::RecordClause { predicates: vec![] });
                assert!(invalid.check().is_err());
                invalid.clauses = vec![harmos::storage::RecordClause {
                    predicates: vec![
                        Predicate::Exists {
                            expr: Expr::Col(0),
                            present: true
                        };
                        65
                    ],
                }];
                assert!(invalid.check().is_err());
                invalid.clauses = vec![
                    harmos::storage::RecordClause {
                        predicates: vec![
                            Predicate::Exists {
                                expr: Expr::Col(0),
                                present: true
                            };
                            33
                        ],
                    };
                    2
                ];
                assert!(invalid.check().is_err());
                invalid = union;
                invalid.clauses[1].predicates.push(Predicate::Exists {
                    expr: Expr::Col(99),
                    present: true,
                });
                assert!(invalid.check().is_err());
                let mut bounded = make(filter.clone());
                bounded.options.decoded_bytes = Some(NonZeroUsize::MIN);
                let mut reader = f.storage.query(bounded).await.unwrap();
                assert!(reader.next_page().await.is_err());
                drop(reader);
                let before = f.data.control.query_views.load(Ordering::SeqCst);
                let mut bounded = make(filter.clone());
                bounded.options.execution = Some(QueryLimits {
                    memory_bytes: NonZeroUsize::MIN,
                    ..f.execution
                });
                assert!(f.storage.query(bounded).await.is_err());
                assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                f.data
                    .control
                    .refuse_record_filters
                    .store(true, Ordering::SeqCst);
                assert!(f.storage.query(make(filter.clone())).await.is_err());
                f.data
                    .control
                    .refuse_record_filters
                    .store(false, Ordering::SeqCst);
                let mut invalid = filter.clone();
                invalid.clauses[0].predicates.push(Predicate::In {
                    expr: Expr::Col(0),
                    values: vec![ScalarValue::Unsigned(2); 65],
                });
                assert!(f.storage.query(make(invalid)).await.is_err());
                assert!(f.storage.follow(make(filter.clone())).await.is_err());
                assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                let mut invalid = filter;
                invalid.columns[1].path =
                    ScalarPath::Payload(vec![Identifier::new("unknown").unwrap()]);
                let mut reader = f.storage.query(make(invalid)).await.unwrap();
                assert!(reader.next_page().await.is_err());
                drop(reader);
                f.finish().await;
            });
        });
    }
    {
        scenario_runtime::check("nested_record_ownership_keeps_cut", || {
            let mut executor = tokio::runtime::Builder::new_current_thread();
            executor.enable_all().build().unwrap().block_on(async {
                let f = Fixture::new(config()).await;
                f.storage
                    .write_schema(
                        &schema(),
                        &harmos::schema::graph::Schema::from_declaration(
                            <Evidence as harmos::schema::Described>::DEFINITION,
                            &harmos::config::SchemaSettings::default(),
                        )
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                for (index, owner) in [
                    Some(Ownership {
                        session: "a".into(),
                        cycle: Some("one".into()),
                    }),
                    Some(Ownership {
                        session: "a".into(),
                        cycle: Some("two".into()),
                    }),
                    Some(Ownership {
                        session: "b".into(),
                        cycle: Some("one".into()),
                    }),
                    Some(Ownership {
                        session: "a".into(),
                        cycle: None,
                    }),
                    None,
                ]
                .into_iter()
                .enumerate()
                {
                    let payload = Evidence { value: 2, owner }.encode_to_vec();
                    f.storage
                        .write_records(&[record(
                            &schema(),
                            &Identifier::new(format!("owned{index}")).unwrap(),
                            &payload,
                        )])
                        .await
                        .unwrap();
                }
                f.storage.flush().await.unwrap();
                let mut query = request();
                for field in ["session", "cycle"] {
                    query.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::Payload(vec![
                            Identifier::new("owner").unwrap(),
                            Identifier::new(field).unwrap(),
                        ]),
                        kind: ScalarKind::Text,
                    });
                }
                query.frame.group_bys[0].aggs = vec![(Expr::Col(0), Agg::Len)];
                query.frame.group_bys[0].filter = vec![
                    Predicate::In {
                        expr: Expr::Col(0),
                        values: vec![ScalarValue::Unsigned(2)],
                    },
                    Predicate::Compare {
                        expr: Expr::Col(1),
                        compare: Compare::Eq,
                        value: ScalarValue::Text("a".into()),
                    },
                    Predicate::Exists {
                        expr: Expr::Col(2),
                        present: true,
                    },
                    Predicate::Compare {
                        expr: Expr::Col(2),
                        compare: Compare::Eq,
                        value: ScalarValue::Text("one".into()),
                    },
                ];
                let mut reader = f.storage.query(query).await.unwrap();
                let saved = reader.view().saved().unwrap();
                let payload = Evidence {
                    value: 2,
                    owner: Some(Ownership {
                        session: "a".into(),
                        cycle: Some("one".into()),
                    }),
                }
                .encode_to_vec();
                f.storage
                    .write_records(&[record(
                        &schema(),
                        &Identifier::new("late").unwrap(),
                        &payload,
                    )])
                    .await
                    .unwrap();
                f.storage.flush().await.unwrap();
                assert!(f.storage.cursor_saved().sequence > saved.sequence);
                let batch = reader.read().await.unwrap();
                assert_eq!(
                    batch.outputs[0].groups[0].values,
                    vec![ScalarValue::Unsigned(1)]
                );
                drop(batch);
                drop(reader);
                f.finish().await;
            });
        });
    }
    {
        scenario_runtime::check("membership_presence_and_ownership", || {
            let mut executor = tokio::runtime::Builder::new_current_thread();
            executor.enable_all().build().unwrap().block_on(async {
                let f = Fixture::new(config()).await;
                let field = harmos::schema::graph::Field {
                    name: "value".into(),
                    tag: 1,
                    kind: harmos::schema::graph::Kind::U64,
                    cardinality: harmos::schema::Cardinality::Optional,
                };
                f.storage
                    .write_schema(
                        &schema(),
                        &harmos::schema::graph::Schema {
                            root: 0,
                            definitions: vec![vec![field]],
                        },
                    )
                    .await
                    .unwrap();
                let owner = Identifier::new("line").unwrap();
                for (index, (value, scoped)) in [
                    (Some(2u64), true),
                    (Some(4), true),
                    (None, true),
                    (Some(2), false),
                ]
                .into_iter()
                .enumerate()
                {
                    let payload = value.encode_to_vec();
                    let item = Identifier::new(format!("value{index}")).unwrap();
                    f.storage
                        .write_records(&[RecordWrite {
                            scope: scoped.then_some(&owner),
                            ..record(&schema(), &item, &payload)
                        }])
                        .await
                        .unwrap();
                }
                f.storage.flush().await.unwrap();
                let make = |filters| {
                    let mut query = request();
                    query.frame.scans[0].columns.push(ScalarField {
                        path: ScalarPath::Scope,
                        kind: ScalarKind::Text,
                    });
                    query.frame.group_bys[0].aggs = vec![(Expr::Col(0), Agg::Len)];
                    query.frame.group_bys[0].filter = filters;
                    query
                };
                for (filters, expected) in [
                    (
                        vec![Predicate::In {
                            expr: Expr::Col(0),
                            values: vec![ScalarValue::Unsigned(2), ScalarValue::Unsigned(2)],
                        }],
                        2,
                    ),
                    (
                        vec![Predicate::In {
                            expr: Expr::Col(0),
                            values: vec![],
                        }],
                        0,
                    ),
                    (
                        vec![Predicate::Exists {
                            expr: Expr::Col(0),
                            present: true,
                        }],
                        3,
                    ),
                    (
                        vec![Predicate::Exists {
                            expr: Expr::Col(0),
                            present: false,
                        }],
                        1,
                    ),
                    (
                        vec![Predicate::Exists {
                            expr: Expr::Col(1),
                            present: false,
                        }],
                        1,
                    ),
                    (
                        vec![
                            Predicate::In {
                                expr: Expr::Col(0),
                                values: vec![ScalarValue::Unsigned(2), ScalarValue::Unsigned(4)],
                            },
                            Predicate::Exists {
                                expr: Expr::Col(1),
                                present: true,
                            },
                            Predicate::Compare {
                                expr: Expr::Col(1),
                                compare: Compare::Eq,
                                value: ScalarValue::Text("line".into()),
                            },
                        ],
                        2,
                    ),
                ] {
                    let query = make(filters);
                    let encoded = query.frame.encode_to_vec();
                    let decoded = Frame::decode(&encoded).unwrap();
                    assert_eq!(decoded, query.frame);
                    let mut reader = f
                        .storage
                        .query(FrameQuery {
                            frame: decoded,
                            options: query.options,
                        })
                        .await
                        .unwrap();
                    let batch = reader.read().await.unwrap();
                    assert_eq!(
                        batch.outputs[0].groups[0].values,
                        vec![ScalarValue::Unsigned(expected)]
                    );
                }
                let before = f.data.control.query_views.load(Ordering::SeqCst);
                for values in [
                    vec![ScalarValue::Unsigned(2); 65],
                    vec![ScalarValue::Text("wrong type".into())],
                    vec![ScalarValue::Float(f64::NAN)],
                ] {
                    assert!(
                        f.storage
                            .query(make(vec![Predicate::In {
                                expr: Expr::Col(0),
                                values
                            }]))
                            .await
                            .is_err()
                    );
                }
                let mut invalid = make(vec![Predicate::Exists {
                    expr: Expr::Col(9),
                    present: true,
                }]);
                assert!(f.storage.query(invalid).await.is_err());
                assert_eq!(f.data.control.query_views.load(Ordering::SeqCst), before);
                let decodes = f.data.control.frame_decodes.load(Ordering::SeqCst);
                invalid = make(vec![Predicate::Exists {
                    expr: Expr::Col(0),
                    present: true,
                }]);
                invalid.frame.scans[0].columns[0].path =
                    ScalarPath::Payload(vec![Identifier::new("unknown").unwrap()]);
                let mut reader = f.storage.query(invalid).await.unwrap();
                assert!(reader.read().await.is_err());
                assert_eq!(f.data.control.frame_decodes.load(Ordering::SeqCst), decodes);
                drop(reader);
                f.finish().await;
            });
        });
    }
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
                    wrong_predicate.frame.group_bys[0]
                        .filter
                        .push(Predicate::Compare {
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
                        right_filter: vec![Predicate::Compare {
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
