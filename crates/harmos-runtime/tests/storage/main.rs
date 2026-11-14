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
mod default_backend;
mod filtered_pages;
#[path = "../support/storage_fixture.rs"]
pub(crate) mod fixture;
mod issued_cut_lifetime;
mod issued_pages;
#[path = "../storage/manager.rs"]
mod manager;
mod native_pages;
#[path = "../storage/operations.rs"]
mod operations;
mod registered_backend;
#[path = "../storage/runtime.rs"]
mod runtime;
#[path = "../storage/state.rs"]
mod state;
use crate::support;
#[path = "../../../harmos-storage/tests/support/waveforms.rs"]
#[allow(dead_code, reason = "shared with the benchmark of the same domain")]
mod waveforms;
use fixture::*;
