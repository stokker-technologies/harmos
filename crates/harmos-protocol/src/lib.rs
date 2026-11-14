/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Audience-separated client, plugin-host and instance protocol contracts.
//! [`schema`] owns descriptor fingerprints and catalog digests shared by peers.

/* === Modules === */

pub mod clock;
pub mod guest;
pub mod policy;
pub mod service;
pub mod transfer;
#[rustfmt::skip]
pub mod client;
#[rustfmt::skip]
pub mod plugin;
#[rustfmt::skip]
mod reflection;
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
