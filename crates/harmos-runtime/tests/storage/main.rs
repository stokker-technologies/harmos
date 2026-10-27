/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../../harmos-storage/tests/support/storage.rs"]
mod backend;
#[path = "../storage/coordinator.rs"]
mod coordinator;
#[path = "../support/storage_fixture.rs"]
pub(crate) mod fixture;
#[path = "../storage/manager.rs"]
mod manager;
#[path = "../storage/operations.rs"]
mod operations;
#[path = "../storage/runtime.rs"]
mod runtime;
#[path = "../storage/state.rs"]
mod state;
#[path = "../support/mod.rs"]
mod support;
#[path = "../../../harmos-storage/tests/support/waveforms.rs"]
#[allow(dead_code, reason = "shared with the benchmark of the same domain")]
mod waveforms;
use fixture::*;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
