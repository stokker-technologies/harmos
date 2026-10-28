/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod contention;
mod directories;
mod files;
mod filesystem;
mod fixture;
mod ingestion;
mod metrics;
mod query;
mod transforms;
#[path = "../../../harmos-storage/tests/support/waveforms.rs"]
mod waveforms;

/* === Re-Exports === */

/* === Imports === */

extern crate harmos_runtime as harmos;

// Standard Library Imports
use std::convert::Infallible;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
// Third Party Imports
use criterion::{BenchmarkId, Criterion, SamplingMode};
use serde::{Deserialize, Serialize};
use serde_json::json;
// Local Imports
use harmos_runtime::storage::*;
use harmos_runtime::{Identifier, Runtime, state::Envelope};

/* === Constants === */

const PAYLOAD_BYTES: usize = 128;
static STATE_CLONES: AtomicUsize = AtomicUsize::new(0);

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Model {
    storage: Option<StorageManager>,
}

#[harmos_runtime::message]
struct Text {
    value: String,
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    count: u64,
}

#[harmos_runtime::transaction(id = "bench.advance", irreversible)]
#[derive(Clone, Serialize, Deserialize)]
struct Advance {
    payload: String,
}

#[harmos_runtime::record(id = "bench.observed")]
#[derive(Clone, Serialize, Deserialize)]
struct Observed {
    payload: String,
}

struct Projection;

/* === Enums === */

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

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

impl harmos_runtime::Lifecycle for Model {
    type Config = ();
    type Error = std::io::Error;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _setup: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self { storage: None })
    }
    async fn initialize(&mut self, context: &Self::Context) -> std::result::Result<(), Self::Error> {
        self.storage = Some(context.resource::<StorageManager>().map_err(std::io::Error::other)?.as_ref().clone());
        Ok(())
    }
    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        if let Some(storage) = &self.storage {
            storage.close().await.map_err(std::io::Error::other)?;
        }
        Ok(())
    }
    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        self.storage.take();
        Ok(())
    }
}

impl harmos_runtime::Application for Model {
    type Lanes = Lanes;
    type State = State;
    type Transaction = Transactions;
    type Record = Records;
    type Streams = ();
}

impl harmos_runtime::state::Transaction<State> for Advance {
    type Error = Infallible;

    fn check(&self, _: &State) -> std::result::Result<(), Infallible> {
        Ok(())
    }

    fn records(&self, _: &State, output: &mut harmos_runtime::state::Records) {
        output.push(Observed { payload: self.payload.clone() }).unwrap();
    }

    fn apply(&self, state: &mut State) {
        state.count += 1;
    }
}

impl RecordProjection for Projection {
    fn project<'a>(
        &'a self, entry: &'a Envelope<&serde_json::Value>, output: &'a mut ProjectedRecords<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = harmos_runtime::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let value = entry.payload.get("payload").and_then(serde_json::Value::as_str).unwrap().to_owned();
            let payload = harmos_runtime::Message::encode(&Text { value });
            let item = Identifier::new(format!("record-{}", entry.position.0)).unwrap();
            output
                .push(RecordInput {
                    schema: schema(),
                    item,
                    payload,
                    scope: None,
                    source: None,
                    source_time: None,
                })
                .await?;
            Ok(())
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    println!("{}", serde_json::json!({"kind":"storage_benchmark_provenance", "provenance": metrics::provenance()}));
    let executor = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    let mut criterion = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(100))
        .measurement_time(Duration::from_millis(1))
        .configure_from_args();
    let mut group = criterion.benchmark_group("storage");
    group.sample_size(10);
    group.sampling_mode(SamplingMode::Flat);
    for count in [50, 100, 200] {
        for mode in ["direct", "direct_grouped", "journal", "captured_record", "transaction"] {
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
    filesystem::run(&mut criterion, &executor);
    directories::run(&mut criterion, &executor);
    contention::run(&mut criterion, &executor);
    criterion.final_summary();
}

async fn measure(mode: &str, count: usize, repetition: Option<usize>) -> Duration {
    let directory = tempfile::tempdir().unwrap();
    let storage: StorageManager = StorageManager::new(fixture::backends(directory.path()), fixture::settings()).unwrap();
    let runtime = Runtime::<Model>::launch_with(State::default(), (), |setup| {
        setup.resource::<StorageManager>(storage.clone());
        setup.settings().storage(&storage);
        setup.settings().register(Transactions::catalog());
        setup.settings().register(Records::catalog());
        if matches!(mode, "journal" | "captured_record" | "transaction") {
            setup.settings().managed_storage(harmos_runtime::config::ManagedStorageSettings {
                replay: limits(1, 64 * 1_024),
                capture: limits(1, 64 * 1_024),
                snapshots: None,
                journal_tail: 1,
            });
        } else {
            setup.settings().tail(1);
        }
        if matches!(mode, "captured_record" | "transaction") {
            setup.settings().store_records(Projection);
        }
    })
    .await
    .unwrap();
    let schema = schema();
    storage
        .write_schema(
            schema.clone(),
            harmos_runtime::schema::graph::Schema::from_declaration(
                <Text as harmos_runtime::schema::Described>::DEFINITION,
                &harmos_runtime::config::SchemaSettings::default(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    storage.flush().await.unwrap();
    storage.flush_recovery().await.unwrap();

    let payload = "x".repeat(PAYLOAD_BYTES);
    let encoded = harmos_runtime::Message::encode(&Text { value: payload.clone() });
    let items: Vec<_> = (0..count).map(|n| Identifier::new(format!("item-{n}")).unwrap()).collect();
    let inputs: Vec<_> = items.iter().map(|item| record(&schema, item, &encoded)).collect();
    STATE_CLONES.store(0, Ordering::Relaxed);
    let begin = Instant::now();
    let mut halves = Vec::new();
    let mut inputs = inputs.into_iter();
    for _ in 0..2 {
        let part: Vec<_> = inputs.by_ref().take(count / 2).collect();
        let half = Instant::now();
        if mode == "direct_grouped" {
            storage.write_records(part).await.unwrap();
        } else {
            for input in part {
                match mode {
                    "direct" => {
                        storage.write_records(vec![input]).await.unwrap();
                    }
                    "transaction" => {
                        runtime.state.commit(Advance { payload: payload.clone() }).await.unwrap();
                    }
                    _ => {
                        runtime.records.publish(Observed { payload: payload.clone() }).await.unwrap();
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
        .query(harmos_runtime::storage::Query {
            at: storage.cursor_saved(),
            source: harmos_runtime::storage::query::Source::Records(harmos_runtime::storage::query::RecordSelection {
                schema,
                scope: None,
                item: None,
                source_start: None,
                source_end: None,
            }),
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
                "recovery_saved_sequence": storage.status().recovery_saved.sequence,

            })
        );
    }
    drop(reader);
    verify_records(&storage, if mode == "journal" { 0 } else { count }, &payload).await;
    assert_eq!(storage.status().recovery_saved.sequence, if mode.starts_with("direct") { 0 } else { count as u64 });
    verify_recovery(&storage).await;
    runtime.shutdown().await.unwrap();

    Duration::from_secs_f64((admission_ms + flush_data_ms + flush_recovery_ms + replay_ms) / 1_000.0)
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

fn record(schema: &SchemaKey, item: &Identifier, payload: &[u8]) -> RecordInput {
    RecordInput {
        schema: schema.clone(),
        scope: None,
        item: item.clone(),
        source: None,
        source_time: None,
        payload: payload.to_vec(),
    }
}

async fn verify_records(storage: &StorageManager, count: usize, payload: &str) {
    let selection = harmos_runtime::storage::query::RecordSelection {
        schema: schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    };
    let mut reader = storage
        .read_records::<Text>(RecordRead {
            at: storage.cursor_saved(),
            selection,
            after: 0,
        })
        .await
        .unwrap();
    let mut checked = 0;
    let mut after = 0;
    while let Some(page) = reader.next().await.unwrap() {
        assert!(page.len() <= fixture::settings().response.batch_rows.get());
        for record in page.iter() {
            assert!(record.position.sequence > after);
            after = record.position.sequence;
            assert_eq!(record.payload.value, payload);
            checked += 1;
        }
    }
    assert_eq!(checked, count);
}

async fn verify_recovery(storage: &StorageManager) {
    let through = storage.status().recovery_saved;
    let mut after = RecoveryPosition {
        history: through.history.clone(),
        sequence: 0,
    };
    let mut count = 0;
    while after.sequence < through.sequence {
        let page = storage.read_recovery(&after, &through, limits(4096, 1024 * 1024)).await.unwrap();
        assert!(!page.entries.is_empty());
        for entry in &page.entries {
            assert_eq!(entry.position.sequence, after.sequence + 1);
            assert!(storage.cursor_saved().covers(&entry.required_data).unwrap());
            serde_json::from_slice::<serde_json::Value>(&entry.payload).unwrap();
            after = entry.position.clone();
            count += 1;
        }
    }
    assert_eq!(count, through.sequence);
}

/* === Tests === */
