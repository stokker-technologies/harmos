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

mod error;
mod fixture;
mod managed;
mod reference;
mod suite;

/* === Re-Exports === */

pub use error::Error;
pub use managed::{Target, run};
pub use reference::serve;

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
