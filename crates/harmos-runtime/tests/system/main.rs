/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Walk The Host Alone, With The Stamped Guest, And With The Stamped Sidecar

#[path = "../../../harmos-storage/tests/support/storage.rs"]
mod backend;
#[path = "../support/storage_fixture.rs"]
mod fixture;
#[cfg(feature = "plugin-guest")]
mod guest;
#[cfg(all(feature = "plugin-guest", feature = "plugin-sidecar"))]
mod guest_contract;
#[path = "../support/report.rs"]
mod report;
mod runtime;
mod schematic;
#[cfg(feature = "plugin-sidecar")]
mod sidecar;
#[path = "../support/mod.rs"]
mod support;

// The Lanes The Storage Fixture Serves Its Operations On
mod operations {
    pub(crate) use super::support::Lanes;
}

extern crate harmos_runtime as harmos;
