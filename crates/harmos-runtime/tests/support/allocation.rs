/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod feeds {
    use serde::{Deserialize, Serialize};

    #[harmos_runtime::stream(description = "A row published into a small ring.", id = "narrow", window = 128)]
    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub struct Narrow(pub u64);

    #[harmos_runtime::stream(
        description = "The same row shape published into a ring five hundred times as large.",
        id = "wide",
        window = 65_536
    )]
    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub struct Wide(pub u64);

    #[harmos_runtime::stream(
        description = "One waveform row whose own source clock is application-declared payload.",
        id = "waveform",
        window = 65_536
    )]
    #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
    pub struct Waveform {
        pub captured_at_ns: u64,
        pub sample: i16,
    }

    #[harmos_runtime::streams]
    pub enum Bank {
        Narrow(Narrow),
        Wide(Wide),
        Waveform(Waveform),
    }
}

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
// Third Party Imports

// Local Imports
use feeds::{Narrow, Waveform, Wide};
use harmos_runtime::{Application, Runtime, state::At, state::Never, streams::Item, streams::Stream};

/* === Constants === */

const PHASES: [(u64, Duration); 7] = [
    (1_024, Duration::from_millis(2)),
    (4_096, Duration::from_millis(2)),
    (8_192, Duration::from_millis(2)),
    (16_384, Duration::from_millis(2)),
    (8_192, Duration::from_millis(2)),
    (4_096, Duration::from_millis(2)),
    (1_024, Duration::ZERO),
];

const ROWS: u64 = 43_008;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/* === Macros === */

thread_local! {
        static MEASURING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/* === Type Aliases === */

/* === Structs === */

struct Probe;

struct Counting;

/* === Enums === */

#[harmos_runtime::lanes]
pub enum Lanes {
    #[default]
    Open,

    #[lane(serial)]
    Serial,

    #[lane(concurrent(2))]
    Concurrent,

    #[lane(unbounded)]
    Unbounded,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Lifecycle for Probe {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl Application for Probe {
    type Lanes = Lanes;
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = feeds::Bank;
}

// The one unsafe block this crate's tests carry: a pass-through allocator is
// the only way to observe allocation counts from safe test code.
#[allow(unsafe_code, reason = "counting allocations requires a global allocator shim")]
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }
}

/* === Public Functions === */

pub async fn streams_r004_publish_cost_is_independent_of_the_window_size() {
    let runtime = measured_runtime().await;

    // Warm both rings past capacity so every measured publish evicts: the
    // steady state is the honest one, because that is where a naive window
    // would pay for its size.
    for value in 0..(Wide::WINDOW as u64 + 512) {
        runtime.streams.publish(Narrow(value));
        runtime.streams.publish(Wide(value));
    }

    let burst = 4_096;
    let narrow = measured(&runtime, Narrow, burst);
    let wide = measured(&runtime, Wide, burst);

    println!(
        "STREAMS-AC006 allocation — {burst} steady-state publishes: \
         window {} = {narrow}, window {} = {wide}",
        Narrow::WINDOW,
        Wide::WINDOW,
    );

    assert_eq!(
        narrow,
        wide,
        "a publish into a window of {} rows allocates exactly as one into a \
         window of {} rows: the ring is reused, never regrown",
        Narrow::WINDOW,
        Wide::WINDOW,
    );

    runtime.shutdown().await.unwrap();
}

pub async fn streams_ac006_concurrent_waveform_measurement() {
    let runtime = measured_runtime().await;

    // Repeat the deterministic allocation proof in the recorded output so
    // the ledger receives one self-contained set of numbers.
    for value in 0..(Wide::WINDOW as u64 + 512) {
        runtime.streams.publish(Narrow(value));
        runtime.streams.publish(Wide(value));
    }
    let burst = 4_096;
    let narrow = measured(&runtime, Narrow, burst);
    let wide = measured(&runtime, Wide, burst);
    assert_eq!(narrow, wide);

    let done = Arc::new(AtomicBool::new(false));
    let mut watching = runtime.streams.watch::<Waveform>();
    let (watch_ready, watch_started) = tokio::sync::oneshot::channel();
    let watched = tokio::spawn(async move {
        watch_ready.send(()).ok();
        let started = Instant::now();
        let mut delivered = 0_u64;
        let mut missed = 0_u64;
        while delivered + missed < ROWS {
            match next(&mut watching).await.expect("the watch stays live") {
                Item::Row(At { value, .. }) => {
                    std::hint::black_box(value);
                    delivered += 1;
                }
                Item::Gap { missed: count } => missed += count,
            }
        }
        (started.elapsed(), delivered, missed)
    });

    let queried_streams = runtime.streams.clone();
    let queried_done = Arc::clone(&done);
    let (query_ready, query_started) = tokio::sync::oneshot::channel();
    let queried = tokio::spawn(async move {
        query_ready.send(()).ok();
        let mut snapshots = 0_u64;
        let mut rows = 0_u64;
        while !queried_done.load(Ordering::Acquire) || snapshots == 0 {
            let snapshot = queried_streams.query::<Waveform>(..).await.expect("the waveform fits inside its declared window");
            let scanned = snapshot.map(std::hint::black_box).count() as u64;
            rows += scanned;
            snapshots += 1;
            tokio::task::yield_now().await;
        }
        (snapshots, rows)
    });

    watch_started.await.expect("the watcher is polling");
    query_started.await.expect("the query loop is polling");
    let published_streams = runtime.streams.clone();
    let published_done = Arc::clone(&done);
    let started = Instant::now();
    let published = std::thread::spawn(move || {
        let mut sequence = 0_u64;
        for (rows, rest) in PHASES {
            for _ in 0..rows {
                published_streams.publish(Waveform {
                    captured_at_ns: sequence * 40_000,
                    sample: (sequence as i16).wrapping_mul(31),
                });
                sequence += 1;
            }
            std::thread::sleep(rest);
        }
        published_done.store(true, Ordering::Release);
    });

    published.join().expect("publishing panics nowhere");
    let publish_elapsed = started.elapsed();
    let (watch_elapsed, delivered, missed) = watched.await.expect("the watcher never panics");
    let (snapshots, queried_rows) = queried.await.expect("the query loop never panics");

    assert_eq!(delivered + missed, ROWS, "watch loss is exactly accounted");
    println!("\nSTREAMS-AC006 — {ROWS} waveform rows in seven ramp phases");
    println!("  publish: {publish_elapsed:.2?}, {:.0} rows/s", ROWS as f64 / publish_elapsed.as_secs_f64());
    println!("  watch: {watch_elapsed:.2?}, {delivered} delivered, {missed} gap rows");
    println!("  query: {snapshots} snapshots, {queried_rows} rows scanned");
    println!(
        "  allocation: {burst} publishes -> {narrow} allocations at window {}, \
         {wide} at window {}\n",
        Narrow::WINDOW,
        Wide::WINDOW,
    );

    runtime.shutdown().await.unwrap();
}

/* === Private Functions === */

fn measured<S: Stream<Probe>>(runtime: &Runtime<Probe>, row: fn(u64) -> S, count: u64) -> u64 {
    ALLOCATIONS.with(|allocations| allocations.set(0));
    MEASURING.with(|measuring| measuring.set(true));
    for value in 0..count {
        runtime.streams.publish(row(value));
    }
    MEASURING.with(|measuring| measuring.set(false));

    ALLOCATIONS.with(Cell::get)
}

fn count_allocation() {
    MEASURING.with(|measuring| {
        if measuring.get() {
            ALLOCATIONS.with(|allocations| allocations.set(allocations.get() + 1));
        }
    });
}

async fn measured_runtime() -> Runtime<Probe> {
    Runtime::<Probe>::launch_with((), (), |setup| {
        setup.settings().register(feeds::Bank::catalog());
    })
    .await
    .expect("a memory-only boot recovers nothing")
}

async fn next<S: Clone + 'static>(watching: &mut (impl futures_core::Stream<Item = Item<S>> + Unpin)) -> Option<Item<S>> {
    std::future::poll_fn(|context| Pin::new(&mut *watching).poll_next(context)).await
}

/* === Tests === */
