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

extern crate harmos_runtime as harmos;

// Standard Library Imports
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use tokio::sync::{Semaphore, mpsc};

// Local Imports
use harmos_runtime::operations::RetryPolicy;
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

fn dispatch_operations_catalog() -> harmos_runtime::operations::Catalog {
    dispatch_operations_catalog_batch().named(concat!(module_path!(), "::DispatchOperations"))
}

fn dispatch_operations_catalog_batch() -> harmos_runtime::__private::Catalog {
    let mut batch = harmos_runtime::__private::Catalog::new();
    batch = batch.entry(dispatch_operation().expect("representable operation"));
    batch
}

#[harmos_runtime::message(harmos_path = harmos_runtime)]
#[derive(harmos_runtime::Request)]
#[harmos_path(harmos_runtime)]
#[request(response = u64)]
#[derive(Clone)]

struct Dispatch {
    wait: bool,
}

static DISPATCH_DESCRIPTION: harmos_runtime::operations::WireDeclaration = harmos_runtime::operations::WireDeclaration::request::<Dispatch>(
    "bench.dispatch",
    harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(30),
        attempt_timeout: Duration::from_secs(30),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    },
);

const DISPATCH_OPERATION: harmos_runtime::operations::OperationSpec<Dispatch> =
    harmos_runtime::operations::OperationSpec::<Dispatch>::new(&DISPATCH_DESCRIPTION);

/* === Traits === */

/* === Implementations === */

async fn dispatch_handler(
    request: Dispatch, ctx: harmos_runtime::operations::FunctionContext<Dispatch>,
) -> Result<harmos_runtime::Response<u64>, std::convert::Infallible> {
    let waiting = ctx.resource::<Waiting>().expect("declared resource is registered");
    if let (true, Some(signals)) = (request.wait, &waiting.0) {
        let _guard = DispatchGuard(signals.clone());
        signals.running.add_permits(1);
        std::future::pending::<()>().await;
    }
    Ok(42.into())
}

fn dispatch_operation() -> ::core::result::Result<harmos_runtime::operations::OperationFactory, harmos_runtime::operations::ServingError> {
    harmos_runtime::operations::OperationFactory::function(
        DISPATCH_OPERATION,
        const { &[harmos_runtime::Resource::of::<Waiting>()] },
        dispatch_handler,
        |_| harmos_runtime::operations::FailureKind::Permanent,
    )
}

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        self.0.stopped.try_send(Instant::now()).unwrap();
    }
}

/* === Public Functions === */

/* === Private Functions === */

struct App;
impl harmos_runtime::Lifecycle for App {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), setup: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
        setup.catalog(dispatch_operations_catalog(), harmos_runtime::operations::Lane::default());
        Ok(Self)
    }
}

impl harmos_runtime::Application for App {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
}

async fn runtime(signals: Option<Arc<Signals>>) -> Runtime<App> {
    Runtime::<App>::launch_with((), (), |setup| {
        setup.resource::<Waiting>(Waiting(signals));
    })
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
    runtime.shutdown().await.expect("runtime cancellation settles");
    let settled = requested.elapsed();
    let mut dispatched = Vec::with_capacity(load as usize);
    for _ in 0..load {
        let stopped = receipts.try_recv().expect("every running owner executes stop");
        dispatched.push(stopped.duration_since(requested));
    }
    for instance in instances {
        assert!(matches!(instance.join().await, Err(Error::OperationRefused(Refusal::Cancelled { .. }))));
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
    executor.block_on(runtime.shutdown()).unwrap();

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
