/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Benchmark Storage Transform Kernels.

/* === Modules === */

mod transforms;

/* === Re-Exports === */

/* === Imports === */

extern crate harmos_runtime as harmos;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

criterion::criterion_group!(benches, transforms::benchmarks);
criterion::criterion_main!(benches);

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
