/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Language-Neutral Harmos Conformance Runner and Reference Plugin.

/* === Modules === */

mod callers;
pub mod conformance;
mod error;
mod fixture;
mod managed;
pub mod product;
mod reference;
mod suite;

/* === Re-Exports === */

extern crate self as application_protocol;
pub mod host {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/application.host.v1.rs"));
    pub mod access {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/application.host.v1.access.rs"));
    }
    pub mod status {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/application.host.v1.status.rs"));
    }
    pub mod jobs {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/application.host.v1.jobs.rs"));
    }
    pub mod plugins {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/application.host.v1.plugins.rs"));
    }
}

mod product_reflection {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sdk/rust/tests/fixtures/client/client-reflection.rs"));
}

pub use error::Error;
pub use managed::{Target, run};
pub use reference::{announce_address, serve};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

pub const RULE_COUNT: usize = suite::CASES.len();

/* === Macros === */

/* === Type Aliases === */

pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
