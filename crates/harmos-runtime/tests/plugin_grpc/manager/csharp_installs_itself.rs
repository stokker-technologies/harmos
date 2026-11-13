/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A C# Plugin Installs Itself

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

/* === Tests === */

#[tokio::test]
async fn csharp_installs_itself() {
    let program = if std::env::var_os("STKR_TEST_PROFILE").is_some() {
        crate::support::fixtures::binary("harmos-csharp-conformance-plugin", None)
    } else {
        let sdk = crate::support::fixtures::workspace().join("sdk/csharp");
        let path = std::fs::read_to_string(sdk.join("artifacts/paths/Debug/Harmos.Conformance.txt"))
            .expect("Build the C# Fixture: mise run build -- --language csharp");
        sdk.join(path.trim_start_matches('\u{feff}').trim())
    };
    super::support::installs_itself(&program, &[]).await;
}
