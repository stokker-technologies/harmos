/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// WebAssembly Component Adapter

/* === Modules === */

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
mod bindings;
mod context;
mod dispatch;
mod registration;

/* === Re-Exports === */

pub use context::Child;
#[doc(hidden)]
pub use dispatch::Adapter;
pub use registration::Registration;

/* === Imports === */

pub(crate) use harmos_protocol::guest::lifecycle_fingerprint;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/// Component world carried by every SDK guest, independent of its declarations.
pub const WIT: &str = include_str!("../../wit/harmos.wit");

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
