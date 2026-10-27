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
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use tokio::sync::{Semaphore, mpsc};

// Local Imports
use harmos_runtime::operations::{RegistrationExt, RetryPolicy};
use harmos_runtime::{Error, Refusal, Runtime};

/* === Constants === */

/* === Macros === */

criterion::criterion_group!(benches, benchmarks);

/* === Type Aliases === */

/* === Structs === */

struct Cancellation {
    stopped: Vec<Duration>,
    settled: Duration,
}

struct Signals {
    running: Semaphore,
    stopped: mpsc::Sender<Instant>,
}

// Waiting Dispatches Hold Their Run Until Cancelled And Report Each Stop.
struct Waiting(Option<Arc<Signals>>);

struct DispatchGuard(Arc<Signals>);

/* === Enums === */

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

#[harmos_runtime::operation(
        resources = [Waiting],
        response = u64,
        id = "bench.dispatch",
        policy = harmos_runtime::OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(30),
            retries: RetryPolicy::Never,
        },
    )]

struct Dispatch {
    wait: bool,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Operation for Dispatch {
    type Error = std::convert::Infallible;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::Response<u64>, Self::Error> {
        let waiting = ctx.resource::<Waiting>();
        if let (true, Some(signals)) = (self.wait, &waiting.0) {
            let _guard = DispatchGuard(signals.clone());
            signals.running.add_permits(1);
            std::future::pending::<()>().await;
        }
        Ok(42.into())
    }
}

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        self.0.stopped.try_send(Instant::now()).unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */

struct App;
impl harmos_runtime::Application for App {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
    type Lanes = Lanes;
}

async fn runtime(signals: Option<Arc<Signals>>) -> Runtime<App> {
    let mut dependencies = harmos_runtime::Dependencies::new();
    dependencies.insert(Arc::new(Waiting(signals))).unwrap();
    Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .with_registration(harmos_runtime::RegistrationBatch::new(dependencies).operation::<Dispatch>().lane(Lanes::Open))
        .runtime()
        .await
        .unwrap()
}

// Startup And Admission Are Outside Both Cancellation Measurements.
async fn cancellation(load: u32) -> Cancellation {
    let (stopped, mut receipts) = mpsc::channel(load as usize);
    let signals = Arc::new(Signals { running: Semaphore::new(0), stopped });
    let runtime = runtime(Some(signals.clone())).await;
    let mut instances = Vec::new();
    for _ in 0..load {
        instances.push(runtime.operation(Dispatch { wait: true }).start().await.unwrap());
    }
    signals.running.acquire_many(load).await.expect("all operations enter run before cancellation").forget();

    let requested = Instant::now();
    runtime.stop().await.expect("runtime cancellation settles");
    let settled = requested.elapsed();
    let mut dispatched = Vec::with_capacity(load as usize);
    for _ in 0..load {
        let stopped = receipts.try_recv().expect("every running owner executes stop");
        dispatched.push(stopped.duration_since(requested));
    }
    for instance in instances {
        assert!(matches!(instance.join().await, Err(Error::OperationRefused(Refusal::Cancelled))));
    }
    Cancellation { stopped: dispatched, settled }
}

fn benchmarks(c: &mut Criterion) {
    let executor = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    let runtime = executor.block_on(runtime(None));
    let mut dispatch = c.benchmark_group("operations/dispatch");
    dispatch.throughput(Throughput::Elements(1));
    dispatch.bench_function("submit_to_join", |b| {
        b.iter_custom(|iterations| {
            executor.block_on(async {
                let started = Instant::now();
                for _ in 0..iterations {
                    let instance = runtime.operation(Dispatch { wait: false }).start().await.unwrap();
                    let result = instance.join().await.unwrap();
                    assert_eq!(*std::hint::black_box(result), 42);
                }
                started.elapsed()
            })
        });
    });
    dispatch.finish();
    executor.block_on(runtime.stop()).unwrap();

    let mut stopping = c.benchmark_group("operations/runtime_cancel");
    stopping.sample_size(20);
    for load in [1, 32, 256] {
        for metric in ["last_stop_hook", "settled"] {
            stopping.bench_with_input(BenchmarkId::new(metric, load), &load, |b, &load| {
                b.iter_custom(|iterations| {
                    executor.block_on(async {
                        let mut elapsed = Duration::ZERO;
                        for _ in 0..iterations {
                            let observation = cancellation(load).await;
                            elapsed += if metric == "last_stop_hook" {
                                observation.stopped.into_iter().max().unwrap()
                            } else {
                                observation.settled
                            };
                        }
                        elapsed
                    })
                });
            });
        }
    }
    stopping.finish();
}

fn main() {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if let Some(index) = arguments.iter().position(|argument| argument == "--latency-samples") {
        let count: usize = arguments
            .get(index + 1)
            .expect("--latency-samples requires a count from 1 through 10000")
            .parse()
            .expect("sample count is an integer");
        assert!((1..=10000).contains(&count), "sample count must be from 1 through 10000");
        latency_samples(count);
    } else {
        benches();
        Criterion::default().configure_from_args().final_summary();
    }
}

fn latency_samples(count: usize) {
    let executor = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    println!(
        "{}",
        serde_json::json!({
            "kind": "configuration", "samples_per_load": count, "workers": 4,
            "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
            "optimized": !cfg!(debug_assertions), "clock": "std::time::Instant",
            "units": "nanoseconds", "storage": false, "carrier": "host-local",
        })
    );
    for load in [1, 32, 256] {
        let mut stops = Vec::with_capacity(count * load as usize);
        let mut last_stops = Vec::with_capacity(count);
        let mut settlements = Vec::with_capacity(count);
        for sample in 0..count {
            let observation = executor.block_on(cancellation(load));
            let stopped: Vec<_> = observation.stopped.iter().map(|duration| u64::try_from(duration.as_nanos()).unwrap()).collect();
            let settled = u64::try_from(observation.settled.as_nanos()).unwrap();
            println!(
                "{}",
                serde_json::json!({
                    "kind": "sample", "load": load, "sample": sample,
                    "stop_hook_ns": stopped, "settled_ns": settled,
                })
            );
            last_stops.push(*stopped.iter().max().unwrap());
            stops.extend(stopped);
            settlements.push(settled);
        }
        for (metric, mut observations) in [("per_owner_stop_hook", stops), ("last_stop_hook", last_stops), ("settled", settlements)] {
            observations.sort_unstable();
            let quantile = |percent: usize| observations[(observations.len() * percent).div_ceil(100) - 1];
            println!(
                "{}",
                serde_json::json!({
                    "kind": "summary", "load": load, "metric": metric,
                    "count": observations.len(), "quantile": "nearest-rank",
                    "p50_ns": quantile(50), "p95_ns": quantile(95),
                    "p99_ns": quantile(99), "max_ns": observations.last().unwrap(),
                })
            );
        }
    }
}

/* === Tests === */
