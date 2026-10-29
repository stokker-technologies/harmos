/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(feature = "sidecar")]
#[path = "../build/main.rs"]
mod build;
#[cfg(feature = "generate-proto")]
#[path = "../proto/main.rs"]
mod proto;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

#[cfg(feature = "generate-proto")]
extern crate harmos_domain as harmos;

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
