/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../tests/support/app/minimal.rs"]
mod app;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};
use tokio::sync::{Semaphore, mpsc};

// Local Imports
use app::App;
use harmos::operations::{Context, Operation, OperationPolicy, RestartPolicy, RetryPolicy};
use harmos::{Error, Refusal, Runtime};

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

struct Dispatching {
    waiting: Option<Arc<Signals>>,
}

/* === Enums === */
#[harmos::lanes]
enum Lanes {
    #[default]
    Open,
}

#[harmos::operation(
        request = Option<Arc<Signals>>,
        response = usize,
        id = "bench.dispatch",
        policy = OperationPolicy {
            timeout: Duration::from_secs(30),
            attempt_timeout: Duration::from_secs(30),
            retries: RetryPolicy::Never,
            restart: RestartPolicy::Never,
        },
    )]
struct Dispatch;

#[harmos::catalog]
enum Operations {
    Dispatch(Dispatch),
}

/* === Traits === */

/* === Implementations === */
impl Operation for Dispatching {
    type Declaration = Dispatch;
    type Resources = ();
    type Error = Infallible;
    type Endpoints = ();
    type Context<'a> = Context<'a>;

    async fn initialize<'ctx>(_: Arc<()>, _: Self::Context<'ctx>) -> Result<Self, Self::Error> {
        Ok(Self { waiting: None })
    }

    async fn run<'ctx>(
        &mut self,
        _: Self::Context<'ctx>,
        waiting: &mut Option<Arc<Signals>>,
        _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::operations::Response<usize>, Self::Error> {
        let result: ::core::result::Result<usize, Self::Error> = async move {
            self.waiting = waiting.clone();
            if let Some(signals) = &self.waiting {
                signals.running.add_permits(1);
                std::future::pending::<()>().await;
            }
            Ok(42)
        }
        .await;
        result.map(::core::convert::Into::into)
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        if let Some(signals) = &self.waiting {
            signals.stopped.try_send(Instant::now()).unwrap();
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn runtime() -> Runtime<App> {
    Runtime::<App>::builder((), ())
        .register(Lanes::catalog())
        .operation::<Dispatching>("benchmark", Lanes::Open, Arc::new(()))
        .runtime()
        .await
        .unwrap()
}

// Startup And Admission Are Outside Both Cancellation Measurements.
async fn cancellation(load: u32) -> Cancellation {
    let runtime = runtime().await;
    let (stopped, mut receipts) = mpsc::channel(load as usize);
    let signals = Arc::new(Signals {
        running: Semaphore::new(0),
        stopped,
    });
    let instances: Vec<_> = (0..load)
        .map(|_| {
            runtime
                .operations
                .submit_as::<Dispatch>(Some(signals.clone()))
                .unwrap()
        })
        .collect();
    tokio::time::timeout(Duration::from_secs(5), signals.running.acquire_many(load))
        .await
        .expect("all operations enter run before cancellation")
        .unwrap()
        .forget();

    let requested = Instant::now();
    tokio::time::timeout(Duration::from_secs(5), runtime.stop())
        .await
        .expect("runtime cancellation settles")
        .unwrap();
    let settled = requested.elapsed();
    let mut dispatched = Vec::with_capacity(load as usize);
    for _ in 0..load {
        let stopped = receipts
            .try_recv()
            .expect("every running owner executes stop");
        dispatched.push(stopped.duration_since(requested));
    }
    for instance in instances {
        assert!(matches!(
            instance.join().await,
            Err(Error::OperationRefused(Refusal::Cancelled))
        ));
    }
    Cancellation {
        stopped: dispatched,
        settled,
    }
}

fn benchmarks(c: &mut Criterion) {
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let runtime = executor.block_on(runtime());
    let mut dispatch = c.benchmark_group("operations/dispatch");
    dispatch.throughput(Throughput::Elements(1));
    dispatch.bench_function("submit_to_join", |b| {
        b.iter_custom(|iterations| {
            executor.block_on(async {
                let started = Instant::now();
                for _ in 0..iterations {
                    let instance = runtime.operations.submit_as::<Dispatch>(None).unwrap();
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
    if let Some(index) = arguments
        .iter()
        .position(|argument| argument == "--latency-samples")
    {
        let count: usize = arguments
            .get(index + 1)
            .expect("--latency-samples requires a count from 1 through 10000")
            .parse()
            .expect("sample count is an integer");
        assert!(
            (1..=10000).contains(&count),
            "sample count must be from 1 through 10000"
        );
        latency_samples(count);
    } else {
        benches();
        Criterion::default().configure_from_args().final_summary();
    }
}

fn latency_samples(count: usize) {
    let executor = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
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
            let stopped: Vec<_> = observation
                .stopped
                .iter()
                .map(|duration| u64::try_from(duration.as_nanos()).unwrap())
                .collect();
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
        for (metric, mut observations) in [
            ("per_owner_stop_hook", stops),
            ("last_stop_hook", last_stops),
            ("settled", settlements),
        ] {
            observations.sort_unstable();
            let quantile =
                |percent: usize| observations[(observations.len() * percent).div_ceil(100) - 1];
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
