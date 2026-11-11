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

pub mod clock;
#[rustfmt::skip]
pub mod host;
#[rustfmt::skip]
mod reflection;
#[rustfmt::skip]
pub mod storage;
pub mod schema;

// Checked-In Output of the Generator Exercised by the Integration Drift Check.
#[rustfmt::skip]
pub mod v1;

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
