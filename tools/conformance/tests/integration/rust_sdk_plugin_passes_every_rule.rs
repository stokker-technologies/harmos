/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::support;
use harmos_conformance::{Target, run};

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

#[tokio::test]
async fn managed_plugin_passes_every_rule() -> Result<(), Box<dyn std::error::Error>> {
    // Locate the SDK-Authored Managed Plugin.
    let plugin = support::binary("harmos-sdk-conformance-plugin", env!("CARGO_BIN_EXE_harmos-sdk-conformance-plugin"));

    // Run the Reference Host Through Every Protocol Rule and Managed Shutdown.
    let passed = run(Target::Executable {
        path: plugin.into(),
        arguments: Vec::new(),
    })
    .await?;

    // Require Every Case, Including Catalog Identity and Cleanup, to Pass.
    assert!(passed, "SDK Plugin Failed Reference Conformance");
    Ok(())
}
