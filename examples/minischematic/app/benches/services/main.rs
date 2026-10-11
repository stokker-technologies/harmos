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
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

// Third Party Imports
use criterion::{BenchmarkId, Criterion, Throughput};

// Local Imports
use minischematic_app::{
    app,
    services::autoroute,
    state::{Net, NetId, NetName, Schematic},
};
use minischematic_contract::autoroute::{Echo, EchoRequest};

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
    let component = harmos::guest::componentize(&module).unwrap();
    let (runtime, mut manager, mut lifecycle) = executor.block_on(async {
        let state = Schematic {
            symbols: BTreeMap::new(),
            nets: BTreeMap::from([(
                NetId(1),
                Net {
                    name: NetName::verbatim("power"),
                    pins: BTreeSet::new(),
                },
            )]),
        };
        let runtime = app::builder(state, ()).runtime().await.unwrap();
        autoroute::register_host(&runtime).unwrap();
        let loaded = autoroute::load(&runtime, &component).unwrap();
        let (mut manager, mut lifecycle) = autoroute::lifecycle(&runtime, loaded).await.unwrap();
        manager.initialize(&mut lifecycle).await.unwrap();
        (runtime, manager, lifecycle)
    });
    let mut group = criterion.benchmark_group("host_guest_operation");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(1));
    for bytes in [64, 1024, 64 * 1024] {
        let text = "x".repeat(bytes);
        group.throughput(Throughput::Bytes((2 * bytes) as u64));
        group.bench_with_input(BenchmarkId::new("round_trip", bytes), &text, |b, text| {
            b.iter(|| {
                executor.block_on(async {
                    let response = runtime
                        .operations
                        .call::<Echo>(EchoRequest { text: text.clone() })
                        .await
                        .unwrap();
                    assert_eq!(&response.text, text);
                    std::hint::black_box(response);
                })
            });
        });
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
