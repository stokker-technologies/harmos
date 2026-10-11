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

// Third Party Imports

// Local Imports
use minibench_probe_sidecar::ProbeBridge;

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
#[tracing::instrument(level = "debug", skip_all)]
async fn main() -> minibench_probe_sidecar::Result<()> {
    harmos::sidecar::run_plugin::<ProbeBridge>(std::num::NonZeroUsize::new(64).unwrap()).await?;
    Ok(())
}

/* === Tests === */
