/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A C++ Plugin Installs Itself

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
async fn cpp_installs_itself() {
    let program = std::env::var_os("HARMOS_CPP_PLUGIN").map(std::path::PathBuf::from).unwrap_or_else(|| {
        super::support::language_binary(
            "harmos-cpp-conformance",
            if cfg!(target_os = "windows") {
                "target/cpp/tests/harmos-cpp-conformance.exe"
            } else {
                "target/cpp/tests/harmos-cpp-conformance"
            },
        )
    });
    super::support::installs_itself(&program, &[]).await;
}
