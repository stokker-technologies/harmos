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
use std::time::Duration;

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};

// Local Imports
use schematic_app::{app, services::guest, state::Schematic};
use schematic_domain::{model::Snapshot, operations::ValidateSchematic};

/* === Constants === */

/* === Macros === */
criterion::criterion_group!(benches, benchmarks);
criterion::criterion_main!(benches);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn benchmarks(criterion: &mut Criterion) {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let path = std::env::var_os("HARMOS_BENCH_GUEST")
        .expect("run mise bench to prepare the guest artifact");
    let module = std::fs::read(path).unwrap();
    let component = harmos_packaging::componentize(&module).unwrap();
    let runtime = executor.block_on(async {
        let runtime = app::builder(Schematic::default(), ())
            .runtime()
            .await
            .unwrap();
        let loaded = guest::load(&runtime, &component).unwrap();
        guest::start(&runtime, loaded).await.unwrap();
        runtime
    });
    let mut group = criterion.benchmark_group("host_guest_operation");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(1));
    for symbols in [8, 128, 8192] {
        let request = ValidateSchematic {
            schematic: Snapshot {
                symbols: (0..symbols).collect(),
                connections: Vec::new(),
            },
        };
        group.throughput(Throughput::Elements(symbols));
        group.bench_with_input(
            BenchmarkId::new("validate", symbols),
            &request,
            |b, request| {
                b.iter(|| {
                    executor.block_on(async {
                        let response = runtime.operations.call(request.clone()).await.unwrap();
                        assert!(response.diagnostics.is_empty());
                        std::hint::black_box(response);
                    })
                });
            },
        );
    }
    group.finish();
    executor.block_on(async {
        runtime.stop().await.unwrap();
    });
}

/* === Tests === */
