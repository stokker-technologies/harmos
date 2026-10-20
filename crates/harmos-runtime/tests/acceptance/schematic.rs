/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Inspect The Stamped Plugin Metadata Through The Cargo Subcommand

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::Path;
use std::process::{Command, Output};

// Third Party Imports
use serde_json::Value;

// Local Imports
use super::fixtures;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

// Cargo Invokes Its Subcommand With The Subcommand Name First
fn cargo_harmos(directory: &Path, arguments: &[&std::ffi::OsStr]) -> Output {
    let tool = fixtures::artifact("cargo-harmos");

    Command::new(tool).current_dir(directory).arg("harmos").args(arguments).output().unwrap()
}

fn succeeded(output: &Output) {
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

/* === Tests === */

#[test]
fn inspect_reports_plugins() {
    let directory = tempfile::tempdir().unwrap();
    for (artifact, facility) in [("schematic-guest", "guest"), ("schematic-sidecar", "sidecar")] {
        let path = fixtures::artifact(artifact);
        let output = cargo_harmos(directory.path(), &[facility.as_ref(), "inspect".as_ref(), path.as_os_str(), "--json".as_ref()]);
        succeeded(&output);

        let record: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(record["facility"], facility);
        assert_eq!(record["id"], "schematic-tools");
        assert_eq!(record["sha256"].as_str().unwrap().len(), 64);
        assert!(!record["operations"].as_array().unwrap().is_empty());
    }
}

#[test]
fn inspect_refuses_inputs() {
    // A Missing File Is An Input Failure; Bytes That Declare No Plugin Are Invalid
    let directory = tempfile::tempdir().unwrap();
    let missing = cargo_harmos(directory.path(), &["sidecar".as_ref(), "inspect".as_ref(), "missing-plugin".as_ref()]);
    assert_eq!(missing.status.code(), Some(3));

    let malformed = directory.path().join("invalid.wasm");
    std::fs::write(&malformed, b"invalid input").unwrap();
    let invalid = cargo_harmos(directory.path(), &["sidecar".as_ref(), "inspect".as_ref(), malformed.as_os_str()]);
    assert_eq!(invalid.status.code(), Some(2));
}
