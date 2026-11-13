/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Measure The Durable Native Path Against The Locked Multi-Day Workload.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports

// Local Imports
use harmos_storage::bench_support::{
    self,
    workload::{ROWS, Workload},
};
use harmos_storage::sqlite::{SqliteBackend, SqliteSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub(super) fn run(runtime: &tokio::runtime::Runtime) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("storage.sqlite");
    let workload = Workload::new(ROWS);
    let settings = SqliteSettings {
        chunk_rows: NonZeroUsize::new(16384).unwrap(),
        chunk_bytes: NonZeroUsize::new(8 * 1024 * 1024).unwrap(),
    };
    bench_support::run_with_reopen(runtime, &workload, || Box::new(SqliteBackend::new(&path, workload.history.clone(), settings).unwrap()));
}

/* === Private Functions === */

/* === Tests === */
