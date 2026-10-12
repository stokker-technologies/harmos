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
use schematic_contract::{
    model::Snapshot,
    operations::{ValidateSchematic, validate::Request},
};

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
    let component = harmos_guest::componentize(&module).unwrap();
    let (runtime, mut manager, mut lifecycle) = executor.block_on(async {
        let runtime = app::builder(Schematic::default(), ())
            .runtime()
            .await
            .unwrap();
        let loaded = guest::load(&runtime, &component).unwrap();
        let (mut manager, mut lifecycle) = guest::lifecycle(&runtime, loaded).await.unwrap();
        manager.initialize(&mut lifecycle).await.unwrap();
        (runtime, manager, lifecycle)
    });
    let mut group = criterion.benchmark_group("host_guest_operation");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(1));
    for symbols in [8, 128, 8192] {
        let request = Request {
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
                        let response = runtime
                            .operations
                            .call::<ValidateSchematic>(request.clone())
                            .await
                            .unwrap();
                        assert!(response.diagnostics.is_empty());
                        std::hint::black_box(response);
                    })
                });
            },
        );
    }
    group.finish();
    executor.block_on(async {
        manager.shutdown(&mut lifecycle).await.unwrap();
        drop(manager);
        drop(lifecycle);
        runtime.stop().await.unwrap();
    });
}

/* === Tests === */
