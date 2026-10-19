/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod throughput_protocol;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use harmos_sidecar::run_plugin;
use throughput_protocol::TransportPlugin;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    run_plugin::<TransportPlugin>(NonZeroUsize::new(8).unwrap()).await
}

/* === Tests === */
