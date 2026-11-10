/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Host Lifecycle Fixture

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_sdk_grpc::{Plugin, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let cleanup = std::env::args().nth(1).expect("Cleanup Evidence Path");
    let plugin = Plugin::new("placement", "1.0.0").on_stop(move || {
        std::fs::write(&cleanup, b"stopped")?;
        Ok(())
    });
    plugin.serve()
}

/* === Tests === */
