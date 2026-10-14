/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;

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
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    harmos::sidecar::run_plugin::<schematic_sidecar::Plugin>(NonZeroUsize::new(4).unwrap()).await?;
    Ok(())
}

/* === Tests === */
