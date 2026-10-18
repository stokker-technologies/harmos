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
use harmos_sidecar::Executable;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn refused(path: impl AsRef<std::path::Path>, why: &str) -> String {
    match Executable::at(path) {
        Ok(_) => panic!("{why}"),
        Err(refusal) => refusal.to_string(),
    }
}

/* === Tests === */

#[test]
fn invalid_path_refused() {
    // Absolute on every platform: a bare `/` is relative to Windows.
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path();
    let missing = directory.join("harmos-nonexistent").join("recorder");
    assert!(refused("relative/recorder", "a relative path is refused").contains("not an absolute path"));
    assert!(refused(&missing, "a missing path is refused").contains("cannot be read"));
    assert!(refused(directory, "a directory is refused").contains("not a file"));
}
