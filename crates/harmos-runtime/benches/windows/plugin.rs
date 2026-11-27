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
use prost_types::Duration;

// Local Imports
use harmos::plugin::{Context, Observer, Plugin, Stream, Window};

/* === Constants === */

pub const MODE: &str = "--window-plugin";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(harmos::Message)]
struct Reading {
    value: f64,
}

/// The Columns of `Reading`.
#[derive(harmos::Message)]
struct Readings {
    value: Vec<f64>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// The Plugin-Qualified Identifier of Benchmark Stream `index`.
pub fn stream(index: usize) -> String {
    format!("window_benchmark.readings_{index}")
}

/// Run `streams` Interval Observers; `observer` Is `window` (Whole Windows) or `sample` (One Sample per Call).
pub fn run(mut arguments: impl Iterator<Item = String>) {
    let streams: usize = arguments.next().unwrap().parse().unwrap();
    let nanos: u64 = arguments.next().unwrap().parse().unwrap();
    let observer = arguments.next().unwrap();
    let interval = Duration {
        seconds: (nanos / 1_000_000_000) as i64,
        nanos: (nanos % 1_000_000_000) as i32,
    };
    Plugin::new("window_benchmark", "1.0.0")
        .register(|ctx| {
            for index in 0..streams {
                let stream = Stream::<Reading>::new(stream(index));
                let reading = if observer == "window" {
                    Observer::window(&stream, async |_: &Context, window: &Window| {
                        Ok(Readings {
                            value: (0..window.count).map(|offset| (window.sequence + offset) as f64).collect(),
                        })
                    })
                } else {
                    Observer::new(&stream, async |context: &Context| Ok(Reading { value: context.sequence() as f64 }))
                };
                ctx.register(stream)?;
                ctx.register(reading.every(interval))?;
            }
            Ok(())
        })
        .unwrap()
        .run()
        .unwrap();
}

/* === Private Functions === */

/* === Tests === */
