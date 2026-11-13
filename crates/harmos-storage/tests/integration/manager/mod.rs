/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Storage Engine Without Runtime Composition

/* === Modules === */

#[allow(dead_code)]
#[path = "../../support/storage.rs"]
pub(crate) mod backend;
mod batching;
mod cancellation;
#[path = "../../support/clock.rs"]
#[allow(dead_code, reason = "shared with runtime deadline stories")]
mod clock;
mod cursors;
mod durable_watch;
#[path = "../../support/events.rs"]
pub(crate) mod events;
mod exhaustion;
mod file_ranges;
mod files;
#[path = "../../support/fixture.rs"]
pub(crate) mod fixture;
mod large_batches_preserve_committed_prefixes;
mod mapping;
#[path = "../../support/mapping.rs"]
pub(crate) mod mapping_fixture;
mod mutations;
mod native_stream_reads;
mod nominal;
#[path = "../../support/nominal.rs"]
mod nominal_fixture;
mod objects;
mod policy;
mod retention;
mod typed_reads;
#[path = "../../support/waveforms.rs"]
pub(crate) mod waveforms;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use fixture::*;
