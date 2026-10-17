/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Run The Stamped Schematic CLI Over Either Backend And Inspect Its Plugins

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// Third Party Imports
use serde_json::Value;

// Local Imports
use super::fixtures;

/* === Constants === */
const REOPENED: &str = "Reopened: 2 symbols, 1 net, 3 route points";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
// Run The CLI From A Caller Directory With One Explicit Backend
fn schematic(directory: &Path, backend: &str, plugin: &Path, advanced: bool) -> Vec<String> {
    let mut command = Command::new(fixtures::artifact("schematic-cli"));
    command.current_dir(directory).arg(backend).arg(plugin);
    if advanced {
        command.arg("--advanced");
    }
    let output = command.output().unwrap();
    succeeded(&output);

    String::from_utf8(output.stdout).unwrap().lines().map(str::to_owned).collect()
}

// Cargo Invokes Its Subcommand With The Subcommand Name First
fn cargo_harmos(directory: &Path, arguments: &[&std::ffi::OsStr]) -> Output {
    let tool = fixtures::artifact("cargo-harmos");

    Command::new(tool).current_dir(directory).arg("harmos").args(arguments).output().unwrap()
}

fn succeeded(output: &Output) {
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

fn walkthrough(backend: &str, plugin: PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let lines = schematic(directory.path(), backend, &plugin, false);

    assert_eq!(lines.first().map(String::as_str), Some("Placed R1 and U1, connected POWER, moved R1"));
    assert!(lines.iter().any(|line| line == "Undid and redid the route"));
    assert_eq!(lines.last().map(String::as_str), Some(REOPENED));
    assert!(!lines.iter().any(|line| line.starts_with("Analysis")));
}

/* === Tests === */
#[test]
fn native_walkthrough_reopens() {
    walkthrough("--sidecar", fixtures::artifact("schematic-sidecar"));
}

#[test]
fn guest_walkthrough_reopens() {
    walkthrough("--guest", fixtures::artifact("schematic-guest"));
}

#[test]
fn backends_report_alike() {
    // Only The Native Backend Lists The Caller's Directories
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("zeta")).unwrap();
    std::fs::create_dir(directory.path().join("alpha")).unwrap();
    std::fs::write(directory.path().join("file.txt"), "not a directory").unwrap();
    let sidecar = fixtures::artifact("schematic-sidecar");
    let guest = fixtures::artifact("schematic-guest");
    let native = schematic(directory.path(), "--sidecar", &sidecar, true);
    let guest = schematic(directory.path(), "--guest", &guest, true);

    let (directories, shared): (Vec<_>, Vec<_>) = native.into_iter().partition(|line| line.starts_with("Native directories:"));
    assert_eq!(directories, [r#"Native directories: ["alpha", "zeta"]"#]);
    assert_eq!(shared, guest);
    for expected in ["Analysis settled: 3 findings", "Cancelled analysis and waited for settlement", REOPENED] {
        assert!(guest.iter().any(|line| line == expected), "missing {expected}");
    }
}

#[test]
fn inspect_reports_plugins() {
    let directory = tempfile::tempdir().unwrap();
    for (artifact, facility) in [("schematic-guest", "guest"), ("schematic-sidecar", "sidecar")] {
        let path = fixtures::artifact(artifact);
        let output = cargo_harmos(directory.path(), &["inspect".as_ref(), path.as_os_str(), "--json".as_ref()]);
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
    let missing = cargo_harmos(directory.path(), &["inspect".as_ref(), "missing-plugin".as_ref()]);
    assert_eq!(missing.status.code(), Some(3));

    let malformed = directory.path().join("invalid.wasm");
    std::fs::write(&malformed, b"invalid input").unwrap();
    let invalid = cargo_harmos(directory.path(), &["inspect".as_ref(), malformed.as_os_str()]);
    assert_eq!(invalid.status.code(), Some(2));
}
