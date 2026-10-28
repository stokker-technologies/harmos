/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Exercise Storage Engine Without Runtime Composition

#[allow(dead_code)]
#[path = "../../support/storage.rs"]
pub(crate) mod backend;
mod batching;
mod cancellation;
mod cursors;
mod exhaustion;
mod files;
pub(crate) mod fixture;
mod mapping;
mod mutations;
mod nominal;
mod objects;
mod retention;
mod typed_reads;
#[path = "../../support/waveforms.rs"]
mod waveforms;

use fixture::*;
