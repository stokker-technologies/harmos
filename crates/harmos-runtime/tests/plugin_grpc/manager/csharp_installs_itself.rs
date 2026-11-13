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
    let program = super::support::language_binary(
        "harmos-csharp-conformance-plugin",
        if cfg!(target_os = "windows") {
            "sdk/csharp/Conformance/bin/Debug/net10.0/harmos-csharp-conformance-plugin.exe"
        } else {
            "sdk/csharp/Conformance/bin/Debug/net10.0/harmos-csharp-conformance-plugin"
        },
    );
    super::support::installs_itself(&program, &[]).await;
}
