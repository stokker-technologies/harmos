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

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use harmos::{Application, Runtime, state::Never, streams::BatchItem};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Probe;

#[harmos::stream(id = "raw", window = 4096)]
#[derive(Clone, Serialize, Deserialize)]
struct Raw {
    value: f64,
    other: f64,
}

#[harmos::stream(id = "measured", window = 4096, batch(samples = 4))]
#[harmos::message]
#[derive(Clone, Serialize, Deserialize, PartialEq)]
struct Measured {
    #[harmos(measure)]
    value: f64,
    #[harmos(measure)]
    other: f64,
}

#[harmos::stream(id = "grouped", window = 4096, batch(samples = 1024))]
#[derive(Clone, Serialize, Deserialize)]
struct Grouped {
    #[harmos(measure)]
    value: f64,
    #[harmos(measure)]
    other: f64,
}

/* === Enums === */
#[harmos::streams]
enum Catalog {
    Raw(Raw),
    Measured(Measured),
    Grouped(Grouped),
}

/* === Traits === */

/* === Implementations === */
impl Application for Probe {
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = Catalog;
    type Resources = ();
}

/* === Public Functions === */
pub async fn raw_and_summary_delivery() {
    let runtime = runtime().await;
    let mut first = runtime.streams.batches::<Measured>();
    let mut second = runtime.streams.batches::<Measured>();
    assert_eq!(*runtime.streams.demand::<Measured>().borrow(), 2);
    for group in [3, 1, 5] {
        for _ in 0..group {
            runtime.streams.publish(Measured {
                value: 2.0,
                other: 9.0,
            });
        }
        assert!(
            !runtime
                .streams
                .query::<Measured>(..)
                .await
                .unwrap()
                .collect::<Vec<_>>()
                .is_empty(),
            "raw rows are immediate even before a statistical group fills"
        );
    }
    for _ in 0..2 {
        let Some(BatchItem::Summary(a)) = first.next().await else {
            panic!("full group")
        };
        let Some(BatchItem::Summary(b)) = second.next().await else {
            panic!("full group")
        };
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "observers share the one computation"
        );
        assert_eq!(a.count, 4);
        assert_eq!(a.fields[0].aggregate(harmos::Aggregate::Mean), Some(2.0));
        assert_eq!(a.fields[1].aggregate(harmos::Aggregate::Mean), Some(9.0));
    }
    runtime.stop().await.unwrap();
    let Some(BatchItem::Summary(tail)) = first.next().await else {
        panic!("partial group")
    };
    assert_eq!(tail.count, 1);
    assert!(tail.partial);
    assert_eq!(first.next().await, None);
}

pub async fn summary_cost() {
    const ROWS: u64 = 500_000;
    let runtime = runtime().await;
    let mut rates = [Vec::new(), Vec::new(), Vec::new()];
    for repetition in 0..6 {
        for mode in if repetition % 2 == 0 {
            [0, 1, 2]
        } else {
            [2, 1, 0]
        } {
            let started = std::time::Instant::now();
            for row in 0..ROWS {
                let value = std::hint::black_box((row % 100) as f64);
                if mode == 1 {
                    runtime.streams.publish(Measured {
                        value,
                        other: value * 2.0,
                    });
                } else if mode == 2 {
                    runtime.streams.publish(Grouped {
                        value,
                        other: value * 2.0,
                    });
                } else {
                    runtime.streams.publish(Raw {
                        value,
                        other: value * 2.0,
                    });
                }
            }
            let rate = ROWS as f64 / started.elapsed().as_secs_f64();
            if repetition > 0 {
                rates[mode].push(rate);
            }
        }
    }
    for (mode, mut values) in rates.into_iter().enumerate() {
        values.sort_by(f64::total_cmp);
        eprintln!(
            "RUNTIME SUMMARIES group={} rows/s median={:.0} min={:.0} max={:.0}; two f64 fields, window4096, 5 repetitions, first triplet warmup excluded",
            [0, 4, 1024][mode],
            values[2],
            values[0],
            values[4]
        );
    }
    runtime.stop().await.unwrap();
}

/* === Private Functions === */
async fn runtime() -> Runtime<Probe> {
    Runtime::<Probe>::builder((), ())
        .register(Catalog::catalog())
        .runtime()
        .await
        .unwrap()
}

/* === Tests === */
