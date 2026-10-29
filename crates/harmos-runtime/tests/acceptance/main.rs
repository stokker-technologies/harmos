/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inspect Built Plugin Metadata Through The Cargo Subcommand.

#[path = "../support/fixtures.rs"]
mod fixtures;
mod schematic;
#[cfg(feature = "grpc")]
mod storage;

extern crate harmos_runtime as harmos;
