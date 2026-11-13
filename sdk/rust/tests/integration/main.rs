/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

#[cfg(feature = "wasm")]
mod guest_registration;

#[cfg(feature = "client")]
mod client_rejects_invalid_completion;
mod host_clock_and_resources;
mod support {
    include!("../support/mod.rs");
}

#[cfg(feature = "client")]
extern crate self as application_protocol;
#[cfg(feature = "client")]
extern crate self as library_protocol;
#[cfg(feature = "client")]
pub mod v1 {
    include!("../fixtures/client/library.v1.rs");
    include!("../fixtures/client/application.v1.rs");
    pub mod plugins {
        include!("../fixtures/client/library.v1.plugins.rs");
    }
    pub mod jobs {
        include!("../fixtures/client/application.v1.jobs.rs");
    }
}
#[cfg(feature = "client")]
mod client_reflection {
    include!("../fixtures/client/client-reflection.rs");
}
#[cfg(feature = "client")]
mod composed_resources {
    include!("../fixtures/client/resources.rs");
}
#[cfg(feature = "client")]
mod client_root;

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
