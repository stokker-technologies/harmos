/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Walk The Host Alone, With The Stamped Guest, And With The Stamped Sidecar

/* === Modules === */

#[path = "../../../harmos-storage/tests/support/storage.rs"]
mod backend;
#[path = "../support/storage_fixture.rs"]
mod fixture;
#[cfg(feature = "plugin-guest")]
mod guest;
#[cfg(all(feature = "plugin-guest", feature = "plugin-sidecar"))]
mod guest_contract;
mod runtime;
mod schematic;
#[cfg(feature = "plugin-sidecar")]
mod sidecar;
#[path = "../support/mod.rs"]
mod support;

// The Lanes The Storage Fixture Serves Its Operations On
mod operations {
    #[harmos_runtime::lanes]
    pub(crate) enum Lanes {
        #[default]
        Open,
    }
}

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
