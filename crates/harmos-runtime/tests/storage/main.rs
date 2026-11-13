/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod definition_receipts;
mod generated_example_hash;
mod generated_native_journey;

mod filtered_pages;
#[path = "generated_capture.rs"]
mod generated_capture;
#[path = "generated_directory.rs"]
mod generated_directory;
#[path = "generated_file_framing.rs"]
mod generated_file_framing;
#[path = "generated_file_upload.rs"]
mod generated_file_upload;
#[path = "generated_status.rs"]
mod generated_status;
mod issued_cut_lifetime;
mod issued_pages;
mod native_pages;
mod transfer_plugin;
mod unary_transfers;

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
use crate::support;
#[path = "../../../harmos-storage/tests/support/waveforms.rs"]
#[allow(dead_code, reason = "shared with the benchmark of the same domain")]
mod waveforms;
use fixture::*;
