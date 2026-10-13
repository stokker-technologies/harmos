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
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion};
use futures_util::FutureExt;
use serde_json::json;

// Local Imports
use super::backend::{Data, Recovery, history};
use harmos::storage::frame::*;
use harmos::storage::reduce::{Agg, ScalarKind, ScalarValue};
use harmos::storage::*;
use harmos::{Identifier, MemorySize, Message, Runtime};

/* === Constants === */
const SIZES: [usize; 3] = [1_000, 100_000, 1_000_000];
const PAGE_ROWS: usize = 4_096;

/* === Macros === */

/* === Type Aliases === */
type Window = (usize, ScanOrder, Vec<ProjectedRow>);

/* === Structs === */
// Pre-Sorted In-Memory Windows; Each Page Clones Its Rows As A Decode Would.
struct Memory<'w> {
    windows: &'w [Window],
    decoded: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Memory<'_> {
    fn prepare(tables: &[Vec<ProjectedRow>], frame: &Frame) -> Vec<Window> {
        let mut windows = Vec::new();
        let mut order = |scan: usize, order: ScanOrder| {
            if windows
                .iter()
                .any(|(source, known, _)| *source == scan && *known == order)
            {
                return;
            }
            let mut rows = tables[scan].clone();
            rows.sort_by(|a, b| order.compare(a, b).unwrap());
            windows.push((scan, order, rows));
        };
        for scan in 0..frame.scans.len() {
            order(scan, ScanOrder::Capture);
        }
        for join in &frame.as_ofs {
            let time = |column| match join.strategy {
                AsOfStrategy::Backward { .. } => Some(column),
                AsOfStrategy::Previous => None,
            };
            order(
                join.left,
                ScanOrder::Keyed {
                    by: join.by.iter().map(|&(left, _)| left).collect(),
                    time: time(join.on.0),
                },
            );
            order(
                join.right,
                ScanOrder::Keyed {
                    by: join.by.iter().map(|&(_, right)| right).collect(),
                    time: time(join.on.1),
                },
            );
        }
        windows
    }
}
impl ScanSource for Memory<'_> {
    fn page(
        &mut self,
        scan: usize,
        order: &ScanOrder,
        cursor: Option<&ScanCursor>,
        limits: ReadLimits,
    ) -> impl Future<Output = harmos::Result<ScanPage>> + Send {
        let offset = cursor.map_or(0, |cursor| {
            usize::from_le_bytes(cursor.token.as_slice().try_into().unwrap())
        });
        let (_, _, rows) = self
            .windows
            .iter()
            .find(|(source, known, _)| *source == scan && known == order)
            .expect("bench window was prepared");
        let end = (offset + limits.rows.get()).min(rows.len());
        self.decoded += end - offset;
        let page = ScanPage {
            rows: rows[offset..end].to_vec(),
            next: (end < rows.len()).then(|| ScanCursor {
                token: end.to_le_bytes().to_vec(),
            }),
        };
        std::future::ready(Ok(page))
    }
}

/* === Public Functions === */
pub(super) fn run(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("storage/frame");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(500));
    let page = super::limits(PAGE_ROWS, 1_048_576);
    for count in SIZES {
        for mode in ["count_mean", "median", "as_of", "repeated_bindings"] {
            let (frame, tables) = fixture(mode, count);
            let explain = frame.explain(page).unwrap();
            let rows: usize = tables.iter().map(Vec::len).sum();

            let windows = Memory::prepare(&tables, &frame);
            let mut memory = Memory {
                windows: &windows,
                decoded: 0,
            };
            let start = Instant::now();
            let collected = execute(&frame, &mut memory, &explain, page);
            let elapsed = start.elapsed();
            let passes = memory.decoded as f64 / rows as f64;
            if mode == "repeated_bindings" {
                assert_eq!(memory.decoded, count, "one pass serves both group-bys");
            }
            println!(
                "HARMOS_BENCHMARK {}",
                json!({
                    "name": format!("storage/frame/{mode}/{count}"),
                    "mean": elapsed.as_secs_f64() * 1000.0, "unit": "ms",
                    "workload": {
                        "rows": rows, "groups": 32, "group_bys": frame.group_bys.len(),
                        "rows_decoded": memory.decoded, "decoded_per_source_row": passes,
                        "peak_bytes": explain.peak_bytes, "page_bytes": explain.page_bytes,
                        "group_state_bytes": explain.group_state_bytes,
                        "median_scratch_bytes": explain.median_scratch_bytes,
                        "output_bytes": explain.output_bytes,
                        "result_resident_bytes": collected.memory_size(),
                        "scope": "in-memory ordered scan source; window sorting excluded; page clones stand in for decoding; no filesystem durability claim"
                    }
                })
            );

            group.bench_with_input(BenchmarkId::new(mode, count), &count, |b, _| {
                b.iter(|| {
                    let mut memory = Memory {
                        windows: &windows,
                        decoded: 0,
                    };
                    let collected = execute(&frame, &mut memory, &explain, page);
                    std::hint::black_box(collected);
                })
            });
        }
    }
    group.finish();
}

pub(super) async fn reuse() {
    let count = 1_000usize;
    let data = Arc::new(Data::new(history()));
    let execution = QueryLimits {
        memory_bytes: super::nonzero(64 * 1024 * 1024),
        timeout: Duration::from_secs(30),
        spill_bytes: 0,
    };
    let settings = StorageSettings {
        flush_interval: Duration::from_secs(3600),
        read_memory_bytes: super::nonzero(256 * 1024 * 1024),
        query_execution: execution,
        query_execution_ceiling: execution,
        ..StorageSettings::default()
    };
    let storage = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            recovery: Arc::new(Recovery::new(history())),
            replicas: Vec::new(),
        },
        settings,
    )
    .unwrap();
    let runtime = Runtime::<super::Model>::builder(super::State::default(), ())
        .runtime_with_storage(&storage)
        .await
        .unwrap();
    let schema = super::schema();
    storage
        .write_schema(
            &schema,
            &harmos::schema::graph::Schema {
                root: 0,
                definitions: vec![vec![harmos::schema::graph::Field {
                    name: "value".into(),
                    tag: 1,
                    kind: harmos::schema::graph::Kind::U64,
                    cardinality: harmos::schema::Cardinality::Required,
                }]],
            },
        )
        .await
        .unwrap();
    for index in 0..count {
        let payload = (index as u64).encode_to_vec();
        let item = Identifier::new(format!("sample{index}")).unwrap();
        storage
            .write_records(&[super::record(&schema, &item, &payload)])
            .await
            .unwrap();
    }
    storage.flush().await.unwrap();

    let mut selection = ReadSelection::new(ReadKind::Records);
    selection.schema = Some(schema);
    let mut frame = Frame::new(super::nonzero(count), std::num::NonZeroUsize::MIN);
    let scan = frame.scan(
        selection,
        vec![ScalarField {
            path: ScalarPath::Payload(vec![Identifier::new("value").unwrap()]),
            kind: ScalarKind::Unsigned,
        }],
    );
    frame.group_by(
        GroupBy::new(scan)
            .agg(Expr::Col(0), Agg::Len)
            .agg(Expr::Col(0), Agg::Mean),
    );
    frame.group_by(GroupBy::new(scan).agg(Expr::Col(0), Agg::Median));
    let mut reader = storage
        .query(FrameQuery {
            frame,
            options: QueryOptions::default(),
        })
        .await
        .unwrap();
    let peak = reader.explain().peak_bytes;
    let start = Instant::now();
    let first = reader.read().await.unwrap();
    let first_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    for _ in 0..64 {
        let value = reader.read().await.unwrap();
        assert!(std::ptr::eq(&*first, &*value));
    }
    let reuse_ms = start.elapsed().as_secs_f64() * 1000.0;
    let decodes = data.control.frame_decodes.load(Ordering::SeqCst);
    assert_eq!(decodes, count);
    println!(
        "HARMOS_BENCHMARK {}",
        json!({
            "name": format!("storage/frame/manager_repeated_bindings/{count}"),
            "mean": reuse_ms, "unit": "ms", "workload": {
                "rows": count, "bindings": 64, "group_bys": 2, "first_read_ms": first_ms,
                "backend_pages": data.control.frame_pages.load(Ordering::SeqCst),
                "source_decodes": decodes, "peak_bytes": peak,
                "reader_output_resident_bytes": first.memory_size(),
                "scope": "controlled original saved backend; one capture pass serves both group-bys; bindings share charged output; no filesystem durability claim"
            }
        })
    );
    drop(first);
    drop(reader);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();
}

/* === Private Functions === */
fn execute(frame: &Frame, memory: &mut Memory, explain: &Explain, page: ReadLimits) -> Collected {
    collect(frame, memory, explain, page, &mut || Ok(()))
        .now_or_never()
        .expect("in-memory collect completes without waiting")
        .unwrap()
}
fn scan() -> Scan {
    let mut selection = ReadSelection::new(ReadKind::Records);
    selection.schema = Some(super::schema());
    Scan {
        selection,
        columns: [
            ScalarKind::Unsigned,
            ScalarKind::Datetime,
            ScalarKind::Unsigned,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| ScalarField {
            path: if kind == ScalarKind::Datetime {
                ScalarPath::SourceTime
            } else {
                ScalarPath::Payload(vec![Identifier::new(format!("column{index}")).unwrap()])
            },
            kind,
        })
        .collect(),
    }
}
fn fixture(mode: &str, count: usize) -> (Frame, Vec<Vec<ProjectedRow>>) {
    let right: Vec<_> = (0..count)
        .map(|index| ProjectedRow {
            capture_order: index as u64 * 2 + 1,
            values: vec![
                ScalarValue::Unsigned(index as u64 % 32),
                ScalarValue::Datetime(index as i128),
                ScalarValue::Unsigned(9_007_199_254_740_993 + index as u64),
            ],
        })
        .collect();
    let mut frame = Frame::new(super::nonzero(count), super::nonzero(32));
    let source = frame.scan(scan().selection, scan().columns);
    let column = |join: Option<usize>| match join {
        Some(join) => Expr::AsOf { join, col: 2 },
        None => Expr::Col(2),
    };
    match mode {
        "count_mean" => {
            frame.group_by(
                GroupBy::new(source)
                    .keys(vec![0])
                    .agg(column(None), Agg::Count)
                    .agg(column(None), Agg::Mean),
            );
        }
        "median" => {
            frame.group_by(
                GroupBy::new(source)
                    .keys(vec![0])
                    .agg(column(None), Agg::Median),
            );
        }
        "repeated_bindings" => {
            frame.group_by(
                GroupBy::new(source)
                    .keys(vec![0])
                    .agg(column(None), Agg::Count)
                    .agg(column(None), Agg::Mean),
            );
            frame.group_by(GroupBy::new(source).agg(column(None), Agg::Median));
        }
        _ => {
            let left: Vec<_> = (0..count / 10)
                .map(|index| ProjectedRow {
                    capture_order: index as u64 * 20 + 2,
                    values: vec![
                        ScalarValue::Unsigned(index as u64 * 10 % 32),
                        ScalarValue::Datetime(index as i128 * 10),
                        ScalarValue::Unsigned(0),
                    ],
                })
                .collect();
            let events = frame.scan(scan().selection, scan().columns);
            let join = frame.join_asof(AsOf {
                left: events,
                right: source,
                by: vec![(0, 0)],
                on: (1, 1),
                strategy: AsOfStrategy::Backward { present: 2 },
                right_filter: vec![],
            });
            frame.group_by(
                GroupBy::new(events)
                    .keys(vec![0])
                    .agg(column(Some(join)), Agg::Count)
                    .agg(column(Some(join)), Agg::Mean),
            );
            return (frame, vec![right, left]);
        }
    }
    (frame, vec![right])
}

/* === Tests === */
