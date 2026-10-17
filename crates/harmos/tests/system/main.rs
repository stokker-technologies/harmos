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
#[path = "../support/storage.rs"]
mod backend;
#[path = "../support/storage_fixture.rs"]
mod fixture;
#[cfg(feature = "plugin-guest")]
mod guest;
mod runtime;
#[cfg(feature = "plugin-sidecar")]
mod sidecar;
#[path = "../support/mod.rs"]
mod support;

// The Lanes The Storage Fixture Serves Its Operations On
mod operations {
    #[harmos::lanes]
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
