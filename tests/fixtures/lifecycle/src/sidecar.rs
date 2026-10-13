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
    harmos::plugin::sidecar::run_plugin::<harmos_lifecycle_fixture::native::Plugin>(
        std::num::NonZeroUsize::new(4).unwrap(),
    )
    .await?;
    Ok(())
}

/* === Tests === */
