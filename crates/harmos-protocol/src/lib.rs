/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Protobuf messages and gRPC bindings for the `harmos.v1` plugin protocol.
//! [`schema`] owns descriptor fingerprints and catalog digests shared by peers.

/* === Modules === */

#[rustfmt::skip]
pub mod access;
#[rustfmt::skip]
pub mod status;
pub mod clock;
pub mod guest;
pub mod policy;
pub mod service;
#[rustfmt::skip]
pub mod host;
#[rustfmt::skip]
mod reflection;
#[rustfmt::skip]
pub mod storage;
pub mod schema;
mod storage_helpers;

// Checked-In Output of the Generator Exercised by the Integration Drift Check.
#[rustfmt::skip]
pub mod v1;

/* === Re-Exports === */

pub use storage_helpers::storage_types;

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
