/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
use std::hint::black_box;
use std::io::Write;

// Third Party Imports
use criterion::Criterion;

// Local Imports
use harmos_storage::{ByteRange, StorageManager};

/* === Public Functions === */

pub fn benchmarks(criterion: &mut Criterion, runtime: &tokio::runtime::Runtime) {
    let fixture = runtime.block_on(super::fixture::Fixture::new(super::fixture::config()));
    let owner = tempfile::tempdir().unwrap();
    let mut source = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(owner.path().join("source")).unwrap();
    let bytes = (0..256_000).map(|index| (index % 251) as u8).collect::<Vec<_>>();
    source.write_all(&bytes).unwrap();
    let mut group = criterion.benchmark_group("integration/opened_files");
    group.bench_function("independent_100000_byte_ranges", |b| b.iter(|| runtime.block_on(read_offsets(&fixture.storage, &source))));
    group.finish();
    runtime.block_on(fixture.finish());
}

/* === Private Functions === */

async fn read_offsets(storage: &StorageManager, source: &std::fs::File) {
    let mut left = storage.read_opened_file(source.try_clone().unwrap(), ByteRange { offset: 0, limit: Some(100_000) }).await.unwrap();
    let mut right =
        storage.read_opened_file(source.try_clone().unwrap(), ByteRange { offset: 128_000, limit: Some(100_000) }).await.unwrap();
    loop {
        let (left, right) = tokio::join!(left.next(), right.next());
        let (left, right) = (left.unwrap(), right.unwrap());
        if left.is_none() && right.is_none() {
            break;
        }
        black_box((left, right));
    }
    black_box((left.completion().unwrap(), right.completion().unwrap()));
}
