/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure Host-Granted Interval Windows Against a Real Rust SDK Plugin Process.

/* === Modules === */

mod plugin;
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

const KILOHERTZ: u64 = 1_000_000;

const MEGAHERTZ: u64 = 1_000;

const STREAMS: [usize; 3] = [1, 16, 128];

/// Windows Measured per Iteration and Stream.
const WINDOWS: u64 = 4;

/// Wall Time One Probe Iteration May Take Before Its Case Counts as Not Sustained.
const PROBE: Duration = Duration::from_secs(30);

/* === Macros === */

criterion::criterion_group!(benches, benchmarks);

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
struct Counter {
    samples: AtomicU64,
    changed: Notify,
}

/// The Product Storage Leg: Count Every Drained Sample and Advance the Cursor.
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
    window: Duration,
    observer: &'a str,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl StreamConsumer<support::Bank> for Count {
    fn resume(&self) -> Cursor {
        self.1
    }

    async fn drain(&mut self, batch: Batch<support::Bank>) -> Cursor {
        let samples = batch.publications().map(|(_, _, row)| count(&row.value.payload)).sum::<u64>();
        self.1 = batch.reaches();
        if samples > 0 {
            self.0.samples.fetch_add(samples, Ordering::SeqCst);
            self.0.changed.notify_waiters();
        }
        self.1
    }
}

impl Host {
    async fn start(case: &Case<'_>) -> Self {
        let counter = Arc::new(Counter::default());
        let (clock, controller) = if case.controlled {
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
            arguments: vec![plugin::MODE.into(), case.streams.to_string().into(), case.interval.to_string().into(), case.observer.into()],
            environment: Default::default(),
        });
        let settings = PlacementSettings {
            connect_timeout: Duration::from_secs(30),
            reconnect_delay: Duration::from_secs(1),
            stop_grace: Duration::from_secs(30),
            stream_window: std::num::NonZeroUsize::new(64).unwrap(),
            interval_window: case.window,
        };
        let attached = runtime.attach_grpc(placement, settings).await.unwrap();
        let host = Self { runtime, controller, counter, attached };
        // Every Stream Has Published Its First Window Before Measurement.
        host.advance(case.samples() / WINDOWS).await;
        host
    }

    fn samples(&self) -> u64 {
        self.counter.samples.load(Ordering::SeqCst)
    }

    async fn advance(&self, target: u64) {
        if let Some(controller) = &self.controller {
            controller.advance_until(Duration::from_secs(1_000_000), || self.samples() >= target).await.unwrap();
            return;
        }
        loop {
            let changed = self.counter.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.samples() >= target {
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
    /// Samples per Iteration: `WINDOWS` Windows of Every Stream.
    fn samples(&self) -> u64 {
        let per_window = self.window.as_nanos() as u64 / self.interval;
        self.streams as u64 * per_window * WINDOWS
    }

    fn measure(&self, group: &mut BenchmarkGroup<'_, WallTime>, executor: &tokio::runtime::Runtime) {
        let samples = self.samples();
        let host = executor.block_on(Host::start(self));
        // A Host That Falls Behind the Declared Rate Grants Ever Longer Windows Until Storage Refuses
        // Them; Such a Case Is Reported as Not Sustained Instead of Measured.
        let probe = executor.block_on(async { tokio::time::timeout(PROBE, host.advance(host.samples() + samples)).await });
        if probe.is_err() {
            println!("windows/{}/{}: not sustained, {} of {} probe samples in {PROBE:?}", self.name, self.streams, host.samples(), samples);
            executor.block_on(host.stop());
            return;
        }
        group.throughput(Throughput::Elements(samples));
        group.bench_function(BenchmarkId::new(self.name, self.streams), |bencher| {
            bencher.iter_custom(|iterations| {
                executor.block_on(async {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let target = host.samples() + samples;
                        let started = Instant::now();
                        host.advance(target).await;
                        elapsed += started.elapsed();
                    }
                    elapsed
                })
            });
        });
        executor.block_on(host.stop());
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some(plugin::MODE) => plugin::serve(arguments),
        _ => {
            benches();
            Criterion::default().configure_from_args().final_summary();
        }
    }
}

/// The Sample Count of One Stored Batch Row: Its Third Postcard Varint, After the Start and Period.
fn count(payload: &[u8]) -> u64 {
    let mut fields = payload.iter();
    let mut varint = || {
        let mut value = 0_u64;
        for shift in (0..64).step_by(7) {
            let byte = *fields.next().expect("a stored batch row");
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                break;
            }
        }
        value
    };
    varint();
    varint();
    varint()
}

fn benchmarks(criterion: &mut Criterion) {
    let executor = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();

    // Host-Granted Windows End to End: Plugin Observer, Samples Call, One Admission per Part and the Storage Drain.
    // A 1 kHz Window Lasts 100 ms (100 Samples); a 1 MHz Window Lasts 10 ms (10,000 Samples).
    let mut group = criterion.benchmark_group("windows");
    group.sample_size(10).sampling_mode(SamplingMode::Flat).warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(3));
    for (name, controlled, interval, window, observer) in [
        ("controlled-1khz-window", true, KILOHERTZ, Duration::from_millis(100), "window"),
        ("controlled-1mhz-window", true, MEGAHERTZ, Duration::from_millis(10), "window"),
        ("real-1khz-window", false, KILOHERTZ, Duration::from_millis(100), "window"),
        ("real-1mhz-window", false, MEGAHERTZ, Duration::from_millis(10), "window"),
        ("controlled-1khz-sample", true, KILOHERTZ, Duration::from_millis(100), "sample"),
        ("real-1khz-sample", false, KILOHERTZ, Duration::from_millis(100), "sample"),
    ] {
        for streams in STREAMS {
            let case = Case {
                name,
                controlled,
                streams,
                interval,
                window,
                observer,
            };
            case.measure(&mut group, &executor);
        }
    }
    group.finish();
}

/* === Tests === */
