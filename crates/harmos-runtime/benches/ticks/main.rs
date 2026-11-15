/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Host-Owned Interval Ticks Against a Real Rust SDK Plugin Process.

/* === Modules === */

mod carrier;
mod plugin;
mod spans;
#[path = "../../tests/support/mod.rs"]
mod support;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

// Third Party Imports
use criterion::{BenchmarkGroup, BenchmarkId, Criterion, SamplingMode, Throughput, measurement::WallTime};
use tokio::sync::Notify;

// Local Imports
use harmos_runtime::{
    Runtime,
    clock::{Clock, Controller},
    plugin::grpc::{Attached, Managed, Placement, PlacementSettings},
    storage::{Batch, Cursor, StreamConsumer},
};

/* === Constants === */

const MILLISECOND: u64 = 1_000_000;

// The Shortest Positive Interval: Each Tick Is Due When the Previous Sample Settles.
const UNPACED: u64 = 1_000;

const STREAMS: [usize; 5] = [1, 4, 16, 64, 128];

const WINDOWS: [usize; 4] = [1, 64, 1024, 65536];

/* === Macros === */

criterion::criterion_group!(benches, benchmarks);

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Counter {
    rows: AtomicU64,
    changed: Notify,
}

/// The Product Storage Leg: Count Every Drained Publication and Advance the Cursor.
struct Count(Arc<Counter>, Cursor);

struct Host {
    runtime: Runtime<support::Bank>,
    controller: Option<Controller>,
    counter: Arc<Counter>,
    attached: Attached,
}

struct Case<'a> {
    name: &'a str,
    controlled: bool,
    streams: usize,
    interval: u64,
    observer: String,
    ticks: u64,
    samples: u64,
    parameter: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StreamConsumer<support::Bank> for Count {
    fn resume(&self) -> Cursor {
        self.1
    }

    async fn drain(&mut self, batch: Batch<support::Bank>) -> Cursor {
        let rows = batch.publications().count() as u64;
        self.1 = batch.reaches();
        if rows > 0 {
            self.0.rows.fetch_add(rows, Ordering::SeqCst);
            self.0.changed.notify_waiters();
        }
        self.1
    }
}

impl Host {
    async fn start(controlled: bool, streams: usize, interval: u64, observer: &str) -> Self {
        let counter = Arc::new(Counter::default());
        let (clock, controller) = if controlled {
            let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000));
            (Some(clock), Some(controller))
        } else {
            (None, None)
        };
        let consumer = Count(counter.clone(), Cursor::ORIGIN);
        let runtime = support::booted(Default::default(), |setup| {
            if let Some(clock) = clock {
                setup.settings().clock(clock);
            }
            setup.settings().attach_streams(consumer);
        })
        .await;
        let placement = Placement::Managed(Managed {
            executable: std::env::current_exe().unwrap(),
            arguments: vec![plugin::MODE.into(), streams.to_string().into(), interval.to_string().into(), observer.into()],
            environment: Default::default(),
        });
        let settings = PlacementSettings {
            connect_timeout: Duration::from_secs(30),
            reconnect_delay: Duration::from_secs(1),
            stop_grace: Duration::from_secs(30),
            stream_window: std::num::NonZeroUsize::new(64).unwrap(),
        };
        let attached = runtime.attach_grpc(placement, settings).await.unwrap();
        let host = Self { runtime, controller, counter, attached };
        // Every Stream Has Published Its First Tick Before Measurement.
        host.advance(streams as u64).await;
        host
    }

    fn rows(&self) -> u64 {
        self.counter.rows.load(Ordering::SeqCst)
    }

    async fn advance(&self, target: u64) {
        if let Some(controller) = &self.controller {
            controller.advance_until(Duration::from_secs(1_000_000), || self.rows() >= target).await.unwrap();
            return;
        }
        loop {
            let changed = self.counter.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.rows() >= target {
                return;
            }
            changed.await;
        }
    }

    async fn stop(self) {
        self.attached.stop("Benchmark Complete").await.unwrap();
        self.runtime.shutdown().await.unwrap();
    }
}

impl Case<'_> {
    fn measure(&self, group: &mut BenchmarkGroup<'_, WallTime>, executor: &tokio::runtime::Runtime) {
        let rows = self.streams as u64 * self.ticks;
        group.throughput(Throughput::Elements(rows * self.samples));
        let mut host = None;
        group.bench_function(BenchmarkId::new(self.name, self.parameter), |bencher| {
            let host =
                host.get_or_insert_with(|| executor.block_on(Host::start(self.controlled, self.streams, self.interval, &self.observer)));
            bencher.iter_custom(|iterations| {
                executor.block_on(async {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let target = host.rows() + rows;
                        let started = Instant::now();
                        host.advance(target).await;
                        elapsed += started.elapsed();
                    }
                    elapsed
                })
            });
        });
        if let Some(host) = host {
            executor.block_on(host.stop());
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some(plugin::MODE) => plugin::serve(arguments),
        Some(carrier::MODE) => carrier::serve(),
        _ => {
            spans::install();
            benches();
            Criterion::default().configure_from_args().final_summary();
        }
    }
}

fn benchmarks(criterion: &mut Criterion) {
    let executor = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    breakdown(&executor);

    // Host-Owned Ticks End to End: Plugin Observer, Samples Call, Admission and Storage Drain.
    let mut group = criterion.benchmark_group("ticks");
    group.sample_size(10).sampling_mode(SamplingMode::Flat).warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    for (name, controlled, interval, observer) in [
        ("controlled-plain", true, MILLISECOND, "plain"),
        ("controlled-time", true, MILLISECOND, "time"),
        ("real-1ms-plain", false, MILLISECOND, "plain"),
        ("real-1ms-time", false, MILLISECOND, "time"),
        ("real-unpaced-plain", false, UNPACED, "plain"),
        ("real-unpaced-time", false, UNPACED, "time"),
    ] {
        for streams in STREAMS {
            let case = Case {
                name,
                controlled,
                streams,
                interval,
                observer: observer.into(),
                ticks: 16,
                samples: 1,
                parameter: streams,
            };
            case.measure(&mut group, &executor);
        }
    }
    group.finish();

    // Each Tick's Single Row Carries a Window of Repeated Values; Throughput Counts Values.
    // The SDK Derive Encodes Repeated Scalars Unpacked, One Tag per Value.
    let mut group = criterion.benchmark_group("ticks-window-row");
    group.sample_size(10).sampling_mode(SamplingMode::Flat).warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    for (name, controlled, streams, interval) in [
        ("controlled-1-stream", true, 1, MILLISECOND),
        ("real-unpaced-1-stream", false, 1, UNPACED),
        ("controlled-128-streams", true, 128, MILLISECOND),
    ] {
        // 128 Retained Windows of 65,536 Values Would Hold Gigabytes in Stream Windows.
        for values in WINDOWS.into_iter().filter(|values| streams == 1 || *values <= 1024) {
            let name = format!("{name}-values");
            let case = Case {
                name: &name,
                controlled,
                streams,
                interval,
                observer: format!("window:{values}"),
                ticks: 16,
                samples: values as u64,
                parameter: values,
            };
            case.measure(&mut group, &executor);
        }
    }
    group.finish();

    // The Samples Call Alone: One Tick Answered by a Window of Samples or One Packed Row.
    let mut group = criterion.benchmark_group("samples-wire");
    group.sample_size(10).sampling_mode(SamplingMode::Flat).warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    for shape in ["repeated", "packed"] {
        for values in WINDOWS {
            group.throughput(Throughput::Elements(values as u64));
            let mut connection = executor.block_on(carrier::Connection::open(&format!("{shape}:{values}")));
            group.bench_function(BenchmarkId::new(shape, values), |bencher| {
                bencher.iter_custom(|iterations| {
                    executor.block_on(async {
                        let started = Instant::now();
                        for tick in 0..iterations {
                            let at = prost_types::Timestamp { seconds: tick as i64, nanos: 0 };
                            std::hint::black_box(connection.tick(at).await);
                        }
                        started.elapsed()
                    })
                });
            });
            executor.block_on(connection.close());
        }
    }
    group.finish();
}

/// Attribute One Tick's Wall Time to the Host's Existing Spans, Outside the Timed Cases.
fn breakdown(executor: &tokio::runtime::Runtime) {
    for (title, streams, observer) in [
        ("controlled clock, 1 stream, plain observer", 1, "plain"),
        ("controlled clock, 1 stream, Time.Get observer", 1, "time"),
        ("controlled clock, 128 streams, Time.Get observer", 128, "time"),
    ] {
        executor.block_on(async {
            let host = Host::start(true, streams, MILLISECOND, observer).await;
            host.advance(host.rows() + 64 * streams as u64).await;
            spans::record(true);
            let rows = 256 * streams as u64;
            let started = Instant::now();
            host.advance(host.rows() + rows).await;
            let wall = started.elapsed();
            spans::record(false);
            spans::report(title, rows, wall);
            host.stop().await;
        });
    }
}

/* === Tests === */
