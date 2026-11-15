/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust SDK Interval Observers Served From the Benchmark Executable

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_types::Duration as ProtoDuration;

// Local Imports
use harmos::plugin::{Context, HostOperation, Observer, Plugin, Stream};

/* === Constants === */

pub const MODE: &str = "--tick-plugin";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos::Message)]
struct Reading {
    value: f64,
}

#[derive(harmos::Message)]
struct Window {
    values: Vec<f64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Serve `streams` Interval Observers; `observer` Is `plain`, `time` or `packed:<values>`.
pub fn serve(mut arguments: impl Iterator<Item = String>) {
    let streams: usize = arguments.next().unwrap().parse().unwrap();
    let nanos: u64 = arguments.next().unwrap().parse().unwrap();
    let observer = arguments.next().unwrap();
    let interval = ProtoDuration {
        seconds: (nanos / 1_000_000_000) as i64,
        nanos: (nanos % 1_000_000_000) as i32,
    };
    let mut plugin = Plugin::new("tick_benchmark", "1.0.0").requires_host(HostOperation::TimeGet);
    for index in 0..streams {
        let id = format!("benchmark.ticks_{index}");
        if let Some(values) = observer.strip_prefix("packed:") {
            let values: usize = values.parse().unwrap();
            let stream = Stream::<Window>::new(id);
            let window = Observer::new(&stream, async move |context: &Context| {
                let first = context.sequence() as f64;
                Ok(Window {
                    values: (0..values).map(|offset| first + offset as f64).collect(),
                })
            })
            .every(interval);
            plugin = plugin.stream(stream).observer(window);
            continue;
        }
        let stream = Stream::<Reading>::new(id);
        let reading = if observer == "time" {
            Observer::new(&stream, async |context: &Context| {
                let now = context.now().await?;
                Ok(Reading { value: now.seconds as f64 })
            })
        } else {
            Observer::new(&stream, async |context: &Context| Ok(Reading { value: context.sequence() as f64 }))
        };
        plugin = plugin.stream(stream).observer(reading.every(interval));
    }
    plugin.serve().unwrap();
}

/* === Private Functions === */

/* === Tests === */
