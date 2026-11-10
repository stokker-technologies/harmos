/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve the Hand-Written Rust Conformance Plugin.

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

#[tokio::main]
async fn main() -> harmos_conformance::Result<()> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let remote = arguments == ["--remote"];
    if !arguments.is_empty() && !remote {
        return Err(harmos_conformance::Error::Invalid("Usage: harmos-conformance-plugin [--remote]".into()));
    }
    harmos_conformance::serve(remote).await
}

/* === Private Functions === */

/* === Tests === */
