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

mod client_rejects_invalid_completion;
mod host_clock_and_resources;
mod support {
    include!("../support/mod.rs");
}

extern crate self as application_protocol;
pub mod v1 {
    include!("../fixtures/client/application.v1.rs");
    pub mod jobs {
        include!("../fixtures/client/application.v1.jobs.rs");
    }
    pub mod plugins {
        include!("../fixtures/client/application.v1.plugins.rs");
    }
}
mod client_reflection {
    include!("../fixtures/client/client-reflection.rs");
}
pub mod composed_resources {
    include!("../fixtures/client/resources.rs");
}
mod client_call_context;
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
