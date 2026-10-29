/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Benchmark Exact Scalar Order Statistics

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion};

use harmos_storage::{
    Aggregate,
    query::{
        Accumulator,
        scalar::{Agg, ScalarValue},
    },
};

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    let mut group = criterion.benchmark_group("integration/order_statistics");
    for n in [2, 3, 4, 11, 13, 45, 100, 2049] {
        for statistic in [
            Aggregate::Median,
            Aggregate::Percentile(95),
            Aggregate::Percentile(99),
            Aggregate::Count,
            Aggregate::Min,
            Aggregate::Max,
            Aggregate::Mean,
            Aggregate::Std,
            Aggregate::Rms,
            Aggregate::Sum,
        ] {
            for phase in ["push", "finish"] {
                let name = format!("{statistic:?}/{phase}");
                group.bench_with_input(BenchmarkId::new(name, n), &n, |b, &n| {
                    b.iter_custom(|iterations| {
                        let mut elapsed = Duration::ZERO;
                        for _ in 0..iterations {
                            let mut accumulator = Accumulator::new(Agg::Of(statistic));
                            let start = Instant::now();
                            for (capture, value) in (0..n).rev().enumerate() {
                                accumulator.push(capture as u64, &ScalarValue::Float(black_box(value as f64))).unwrap();
                            }
                            let pushed = start.elapsed();
                            let start = Instant::now();
                            let answer = black_box(accumulator.finish(&mut || Ok(())).unwrap());
                            let finished = start.elapsed();
                            let fraction = match statistic {
                                Aggregate::Median => Some(0.5),
                                Aggregate::Percentile(rank) => Some(f64::from(rank) / 100.0),
                                _ => None,
                            };
                            if let Some(fraction) = fraction {
                                assert_eq!(answer, ScalarValue::Float((n - 1) as f64 * fraction));
                            }
                            elapsed += if phase == "push" { pushed } else { finished };
                        }
                        elapsed
                    });
                });
            }
        }
    }
    group.finish();
    criterion.final_summary();
}
