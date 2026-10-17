/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[allow(
    dead_code,
    reason = "Reuse the controlled backend without its fault scenarios"
)]
#[path = "../../tests/support/storage.rs"]
mod backend;
mod contention;
mod files;
mod fixture;
mod ingestion;
mod metrics;
mod query;
mod transforms;
#[path = "../../tests/support/storage/waveforms.rs"]
mod waveforms;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::convert::Infallible;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
// Third Party Imports
use criterion::{BenchmarkId, Criterion};
use serde::{Deserialize, Serialize};
use serde_json::json;
// Local Imports
use backend::{Data, Recovery, history};
use harmos_runtime::storage::*;
use harmos_runtime::{Identifier, Runtime, state::Envelope};

/* === Constants === */
const PAYLOAD_BYTES: usize = 128;
static STATE_CLONES: AtomicUsize = AtomicUsize::new(0);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Model;

#[harmos_runtime::message]
struct Text {
    value: String,
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    count: u64,
}

#[harmos_runtime::transaction(id = "bench.advance", version = 1, irreversible)]
#[derive(Clone, Serialize, Deserialize)]
struct Advance {
    payload: String,
}

#[harmos_runtime::record(id = "bench.observed", version = 1)]
#[derive(Clone, Serialize, Deserialize)]
struct Observed {
    payload: String,
}

struct Projection;

/* === Enums === */
#[harmos_runtime::transactions(state = State, error = Infallible)]
#[derive(Clone)]
enum Transactions {
    Advance(Advance),
}

#[harmos_runtime::records]
#[derive(Clone)]
enum Records {
    Observed(Observed),
}

/* === Traits === */

/* === Implementations === */
impl Clone for State {
    fn clone(&self) -> Self {
        STATE_CLONES.fetch_add(1, Ordering::Relaxed);
        Self { count: self.count }
    }
}

impl harmos_runtime::Application for Model {
    type State = State;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = ();
    type Resources = ();
}

impl harmos_runtime::state::Transaction<State> for Advance {
    type Error = Infallible;

    fn check(&self, _: &State) -> Result<(), Infallible> {
        Ok(())
    }

    fn records(&self, _: &State, output: &mut harmos_runtime::state::Records) {
        output
            .push(Observed {
                payload: self.payload.clone(),
            })
            .unwrap();
    }

    fn apply(&self, state: &mut State) {
        state.count += 1;
    }
}

impl RecordProjection for Projection {
    fn project<'a>(
        &'a self,
        entry: &'a Envelope<&serde_json::Value>,
        output: &'a mut ProjectedRecords<'_>,
    ) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let payload = serde_json::to_vec(entry).unwrap();
            let item = Identifier::new(format!("record-{}", entry.position.0)).unwrap();
            output.push(record(&schema(), &item, &payload)).await?;
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn main() {
    println!(
        "{}",
        serde_json::json!({"kind":"storage_benchmark_provenance", "provenance": metrics::provenance()})
    );
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    executor.block_on(async {
        for repetition in 0..3 {
            for count in [500, 1_000, 2_000] {
                for mode in [
                    "direct",
                    "direct_grouped",
                    "journal",
                    "captured_record",
                    "transaction",
                ] {
                    measure(mode, count, Some(repetition)).await;
                }
            }
        }
    });

    let mut criterion = Criterion::default().configure_from_args();
    let mut group = criterion.benchmark_group("storage");
    group.sample_size(10);
    for count in [500, 1_000, 2_000] {
        for mode in [
            "direct",
            "direct_grouped",
            "journal",
            "captured_record",
            "transaction",
        ] {
            group.bench_with_input(BenchmarkId::new(mode, count), &count, |b, &count| {
                b.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        for _ in 0..iterations {
                            elapsed += measure(mode, count, None).await;
                        }
                        elapsed
                    })
                });
            });
        }
    }
    group.finish();
    ingestion::run(&mut criterion, &executor);
    query::run(&mut criterion, &executor);
    transforms::run(&mut criterion, &executor);
    files::run(&mut criterion, &executor);
    contention::run(&mut criterion, &executor);
    criterion.final_summary();
}

async fn measure(mode: &str, count: usize, repetition: Option<usize>) -> Duration {
    let data = Arc::new(Data::new(history()));
    let recovery = Arc::new(Recovery::new(history()));
    let config = StorageSettings {
        // The Controlled Backend Pins Whole Tables Before Producing A Page.
        budget: {
            let mut settings = backend::storage_settings().budget;
            settings.read_memory_bytes = nonzero(32 * 1_024 * 1_024);
            settings
        },
        request: {
            let mut settings = backend::storage_settings().request;
            settings.max_pending_write_batches = nonzero(8_192);
            settings.max_pending_write_bytes = nonzero(128 * 1_024 * 1_024);
            settings
        },
        ..backend::storage_settings()
    };
    let storage: StorageManager = StorageManager::new(
        StorageBackends {
            primary: data.clone(),
            state: recovery.clone(),
        },
        config,
    )
    .unwrap();
    let mut builder = Runtime::<Model>::builder(State::default(), ())
        .register(Transactions::catalog())
        .register(Records::catalog())
        .tail(1);
    if matches!(mode, "captured_record" | "transaction") {
        builder = builder
            .managed_state(limits(1, 64 * 1_024))
            .store_records(limits(1, 64 * 1_024), Projection);
    }
    let runtime = builder.runtime_with_storage(&storage).await.unwrap();
    let schema = schema();
    storage
        .write_schema(
            &schema,
            &harmos_runtime::schema::graph::Schema::from_declaration(
                <Text as harmos_runtime::schema::Described>::DEFINITION,
                &harmos_runtime::config::SchemaSettings::default(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    storage.flush().await.unwrap();
    storage.flush_recovery().await.unwrap();
    data.control.write_sizes.lock().unwrap().clear();
    data.control.flushes.lock().unwrap().clear();
    recovery.control.write_sizes.lock().unwrap().clear();
    recovery.control.flushes.lock().unwrap().clear();

    let payload = "x".repeat(PAYLOAD_BYTES);
    let encoded = harmos_runtime::Message::encode_to_vec(&Text {
        value: payload.clone(),
    });
    let items: Vec<_> = (0..count)
        .map(|n| Identifier::new(format!("item-{n}")).unwrap())
        .collect();
    let inputs: Vec<_> = items
        .iter()
        .map(|item| record(&schema, item, &encoded))
        .collect();
    STATE_CLONES.store(0, Ordering::Relaxed);
    let begin = Instant::now();
    let mut halves = Vec::new();
    for part in inputs.chunks(count / 2) {
        let half = Instant::now();
        if mode == "direct_grouped" {
            storage.write_records(part).await.unwrap();
        } else {
            for input in part {
                match mode {
                    "direct" => {
                        storage
                            .write_records(std::slice::from_ref(input))
                            .await
                            .unwrap();
                    }
                    "transaction" => {
                        runtime
                            .state
                            .commit(Advance {
                                payload: payload.clone(),
                            })
                            .await
                            .unwrap();
                    }
                    _ => {
                        runtime
                            .records
                            .publish(Observed {
                                payload: payload.clone(),
                            })
                            .await
                            .unwrap();
                    }
                }
            }
        }
        halves.push(half.elapsed().as_secs_f64() * 1_000.0);
    }
    let admission_ms = begin.elapsed().as_secs_f64() * 1_000.0;
    let state_clones = STATE_CLONES.load(Ordering::Relaxed);
    let pending_bytes = storage.status().pending_bytes;
    let begin = Instant::now();
    storage.flush().await.unwrap();
    let flush_data_ms = begin.elapsed().as_secs_f64() * 1_000.0;
    let begin = Instant::now();
    storage.flush_recovery().await.unwrap();
    let flush_recovery_ms = begin.elapsed().as_secs_f64() * 1_000.0;
    let begin = Instant::now();
    let mut reader = storage
        .query(harmos_runtime::storage::operations::Query {
            at: storage.cursor_saved(),
            source: harmos_runtime::storage::query::Source::Records(
                harmos_runtime::storage::query::RecordSelection {
                    schema,
                    scope: None,
                    item: None,
                    source_start: None,
                    source_end: None,
                },
            ),
            transforms: Vec::new(),
            slice: harmos_runtime::storage::query::RowSlice::default(),
        })
        .await
        .unwrap();
    let mut replay_count = 0;
    while let Some(output) = reader.next().await.unwrap() {
        if let harmos_runtime::storage::query::QueryArrow::Batch(batch) = output {
            replay_count += batch.as_record_batch().num_rows();
        }
    }
    let stored_payload_bytes = replay_count * encoded.len();
    let replay_ms = begin.elapsed().as_secs_f64() * 1_000.0;
    assert_eq!(replay_count, if mode == "journal" { 0 } else { count });
    assert_eq!(storage.status().pending_bytes, 0);
    if let Some(repetition) = repetition {
        println!(
            "{}",
            json!({
                "mode": mode, "count": count, "repetition": repetition,
                "payload_bytes_per_record": PAYLOAD_BYTES,
                "stored_payload_bytes": stored_payload_bytes, "pending_bytes": pending_bytes,
                "admission_ms": admission_ms, "half_admission_ms": halves,
                "flush_data_ms": flush_data_ms, "flush_recovery_ms": flush_recovery_ms,
                "saved_replay_ms": replay_ms, "state_clone_calls": state_clones,
                "data_write_sizes": *data.control.write_sizes.lock().unwrap(),
                "recovery_write_sizes": *recovery.control.write_sizes.lock().unwrap(),
                "data_flush_calls": data.control.flushes.lock().unwrap().len(),
                "recovery_flush_calls": recovery.control.flushes.lock().unwrap().len(),
                "recovery_entries": recovery.entries.lock().unwrap().len(),
            })
        );
    }
    drop(reader);
    storage.close().await.unwrap();
    runtime.stop().await.unwrap();

    Duration::from_secs_f64(
        (admission_ms + flush_data_ms + flush_recovery_ms + replay_ms) / 1_000.0,
    )
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn limits(rows: usize, bytes: usize) -> ReadLimits {
    ReadLimits {
        rows: nonzero(rows),
        bytes: nonzero(bytes),
    }
}

fn schema() -> SchemaKey {
    SchemaKey {
        payload_encoding: harmos_runtime::storage::PayloadEncoding::PostcardV1,
        id: Identifier::new("bench.observed").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
    }
}

fn record<'a>(schema: &'a SchemaKey, item: &'a Identifier, payload: &'a [u8]) -> RecordWrite<'a> {
    RecordWrite {
        schema,
        scope: None,
        item,
        source: None,
        source_time: None,
        payload,
    }
}

/* === Tests === */
