/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// A Python Plugin Installs Itself

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
async fn python_installs_itself() {
    let root = crate::support::fixtures::workspace();
    let program =
        root.join(if cfg!(target_os = "windows") { "sdk/python/.venv/Scripts/python.exe" } else { "sdk/python/.venv/bin/python" });
    let fixture = root.join("sdk/python/tests/fixtures/conformance/__main__.py");
    super::support::installs_itself(&program, &[fixture.to_str().unwrap()]).await;
}
