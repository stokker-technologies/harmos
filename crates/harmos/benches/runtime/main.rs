/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../tests/support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
// Third Party Imports
use tokio::sync::RwLock;
// Local Imports
use harmos::state::Transaction;
use support::{Change, HOUSE, Ledger, deposit, runtime_from};

/* === Constants === */
const WORDS: usize = 131_072;

const COMMITS: usize = 200;

const READERS: usize = 4;

/* === Macros === */
criterion::criterion_group!(benches, benchmarks);

criterion::criterion_main!(benches);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
async fn in_place_writer_against_deleted_clone_and_publish_path() {
    let state = WORDS * size_of::<u64>();
    println!("\nRUNTIME-AC006 — {state} byte state, {COMMITS} commits\n");

    let quiet_writer = write_heavy_in_place().await;
    let quiet_baseline = write_heavy_cloning().await;
    let (busy_writer, writer_reads) = concurrent_in_place().await;
    let (busy_baseline, baseline_reads) = concurrent_cloning().await;

    report("write-heavy, in-place writer", quiet_writer, None);
    report("write-heavy, clone-and-publish", quiet_baseline, None);
    report(
        "concurrent-read, in-place writer",
        busy_writer,
        Some(writer_reads),
    );
    report(
        "concurrent-read, clone-and-publish",
        busy_baseline,
        Some(baseline_reads),
    );
    println!(
        "  read contention: in-place commits are {:.2}x slower with {READERS} readers, \
         clone-and-publish {:.2}x",
        ratio(busy_writer, quiet_writer),
        ratio(busy_baseline, quiet_baseline)
    );
    println!(
        "  write cost: in-place is {:.2}x the clone-and-publish path when quiet, \
         {:.2}x when read\n",
        ratio(quiet_writer, quiet_baseline),
        ratio(busy_writer, busy_baseline)
    );
}

async fn write_heavy_in_place() -> Duration {
    let runtime = runtime_from(Ledger::bulky(WORDS)).await;
    let started = Instant::now();

    for _ in 0..COMMITS {
        runtime.state.commit(deposit(1)).await.expect("accepted");
    }
    let elapsed = started.elapsed();
    runtime.stop().await.unwrap();

    elapsed
}

async fn write_heavy_cloning() -> Duration {
    let published = Arc::new(RwLock::new(Arc::new(Ledger::bulky(WORDS))));
    let started = Instant::now();

    for _ in 0..COMMITS {
        publish(&published, &deposit(1)).await;
    }
    started.elapsed()
}

async fn concurrent_in_place() -> (Duration, u64) {
    let runtime = runtime_from(Ledger::bulky(WORDS)).await;
    let (stop, reads) = signals();
    // Readers hold their own handle: the journal is the cheap cloneable thing,
    // and every clone shares the one reality the writer mutates in place.
    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let journal = runtime.state.clone();
            let stop = Arc::clone(&stop);
            let reads = Arc::clone(&reads);

            tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    if journal
                        .state()
                        .read(|ledger| ledger.balance(HOUSE))
                        .await
                        .is_ok()
                    {
                        reads.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    let started = Instant::now();
    for _ in 0..COMMITS {
        runtime.state.commit(deposit(1)).await.expect("accepted");
    }
    let elapsed = started.elapsed();

    stop.store(true, Ordering::Relaxed);
    for reader in readers {
        reader.await.expect("readers never panic");
    }
    runtime.stop().await.unwrap();

    (elapsed, reads.load(Ordering::Relaxed))
}

async fn concurrent_cloning() -> (Duration, u64) {
    let published = Arc::new(RwLock::new(Arc::new(Ledger::bulky(WORDS))));
    let (stop, reads) = signals();
    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let published = Arc::clone(&published);
            let stop = Arc::clone(&stop);
            let reads = Arc::clone(&reads);

            tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    let snapshot = Arc::clone(&*published.read().await);
                    std::hint::black_box(snapshot.balance(HOUSE));
                    reads.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    let started = Instant::now();
    for _ in 0..COMMITS {
        publish(&published, &deposit(1)).await;
    }
    let elapsed = started.elapsed();

    stop.store(true, Ordering::Relaxed);
    for reader in readers {
        reader.await.expect("readers never panic");
    }

    (elapsed, reads.load(Ordering::Relaxed))
}

async fn publish(slot: &RwLock<Arc<Ledger>>, change: &Change) {
    let mut next = (**slot.read().await).clone();
    change.check(&next).expect("accepted");
    change.apply(&mut next);
    *slot.write().await = Arc::new(next);
}

fn signals() -> (Arc<AtomicBool>, Arc<AtomicU64>) {
    (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicU64::new(0)),
    )
}

fn report(what: &str, elapsed: Duration, reads: Option<u64>) {
    let each = elapsed.as_secs_f64() * 1_000_000.0 / COMMITS as f64;
    let read = reads.map_or_else(String::new, |reads| format!(", {reads} reads served"));

    println!("  {what}: {elapsed:.2?} total, {each:.1} µs/commit{read}");
}

fn ratio(measured: Duration, against: Duration) -> f64 {
    measured.as_secs_f64() / against.as_secs_f64().max(f64::MIN_POSITIVE)
}

fn benchmarks(c: &mut criterion::Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(in_place_writer_against_deleted_clone_and_publish_path());
    let mut group = c.benchmark_group("writer");
    group.sample_size(20);
    group.throughput(criterion::Throughput::Elements(COMMITS as u64));
    for mode in [
        "quiet_in_place",
        "quiet_clone",
        "readers_in_place",
        "readers_clone",
    ] {
        group.bench_function(mode, |b| {
            b.iter_custom(|iterations| {
                runtime.block_on(async {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        elapsed += match mode {
                            "quiet_in_place" => write_heavy_in_place().await,
                            "quiet_clone" => write_heavy_cloning().await,
                            "readers_in_place" => concurrent_in_place().await.0,
                            "readers_clone" => concurrent_cloning().await.0,
                            _ => unreachable!(),
                        };
                    }
                    elapsed
                })
            });
        });
    }
    group.finish();
}

/* === Tests === */
