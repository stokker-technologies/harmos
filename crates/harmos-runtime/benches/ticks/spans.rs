/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Aggregate Existing Host Span Lifetimes by Name and Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

// Third Party Imports
use tracing::{
    Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id},
    subscriber::Interest,
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

// Local Imports

/* === Constants === */

static RECORDING: AtomicBool = AtomicBool::new(false);

static TOTALS: Mutex<BTreeMap<String, (u64, Duration)>> = Mutex::new(BTreeMap::new());

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Spans;

struct Timing {
    started: Instant,
    key: String,
}

struct Operation(Option<String>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Visit for Operation {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "operation" {
            self.0 = Some(format!("{value:?}"));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "operation" {
            self.0 = Some(value.to_owned());
        }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Spans {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if metadata.target().starts_with("harmos") { Interest::sometimes() } else { Interest::never() }
    }

    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        RECORDING.load(Ordering::Relaxed) && metadata.is_span() && metadata.target().starts_with("harmos")
    }

    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, context: Context<'_, S>) {
        let Some(span) = context.span(id) else { return };
        let mut operation = Operation(None);
        attributes.record(&mut operation);
        // A Span Without Its Own Operation Belongs to Its Nearest Annotated Ancestor.
        let inherited = span.parent().and_then(|parent| {
            parent.extensions().get::<Timing>().and_then(|timing| timing.key.split_once(" @ ").map(|(_, operation)| operation.to_owned()))
        });
        let key = match operation.0.or(inherited) {
            Some(operation) => format!("{} @ {operation}", span.name()),
            None => span.name().to_owned(),
        };
        span.extensions_mut().insert(Timing { started: Instant::now(), key });
    }

    fn on_close(&self, id: Id, context: Context<'_, S>) {
        let Some(span) = context.span(&id) else { return };
        let extensions = span.extensions();
        let Some(timing) = extensions.get::<Timing>() else { return };
        let elapsed = timing.started.elapsed();
        let mut totals = TOTALS.lock().unwrap();
        let entry = totals.entry(timing.key.clone()).or_default();
        entry.0 += 1;
        entry.1 += elapsed;
    }
}

/* === Public Functions === */

pub fn install() {
    use tracing_subscriber::layer::SubscriberExt;
    tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Spans)).unwrap();
}

pub fn record(enabled: bool) {
    if enabled {
        TOTALS.lock().unwrap().clear();
    }
    RECORDING.store(enabled, Ordering::Relaxed);
}

/// Print Each Span's Count and Wall Time per Tick; Nested Spans Overlap Their Parents.
pub fn report(title: &str, ticks: u64, wall: Duration) {
    let totals = std::mem::take(&mut *TOTALS.lock().unwrap());
    let per_tick = |duration: Duration| duration.as_secs_f64() * 1e6 / ticks as f64;
    println!("\nTICK BREAKDOWN — {title}: {ticks} ticks, {:.1} µs wall per tick", per_tick(wall));
    println!("  {:<72} {:>10} {:>14}", "span @ operation", "per tick", "µs per tick");
    let mut rows: Vec<_> = totals.into_iter().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.1.1));
    for (key, (count, total)) in rows {
        println!("  {key:<72} {:>10.2} {:>14.1}", count as f64 / ticks as f64, per_tick(total));
    }
}

/* === Private Functions === */

/* === Tests === */
