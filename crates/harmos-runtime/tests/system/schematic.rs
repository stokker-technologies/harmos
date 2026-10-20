/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Walk The CLI Across Both Built Plugin Backends

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::process::Command;

// Third Party Imports

// Local Imports
use super::support::fixtures;

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

/* === Tests === */

#[test]
fn cli_backends_reopen() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("zeta")).unwrap();
    std::fs::create_dir(directory.path().join("alpha")).unwrap();
    std::fs::write(directory.path().join("file.txt"), "not a directory").unwrap();
    let cli = fixtures::artifact("schematic-cli");
    let mut advanced_outputs = Vec::new();

    for (backend, artifact) in [("--sidecar", "schematic-sidecar"), ("--guest", "schematic-guest")] {
        let plugin = fixtures::artifact(artifact);

        // Nominal Editing Persists Through The Backend And Reopens.
        let output = Command::new(&cli).current_dir(directory.path()).arg(backend).arg(&plugin).output().unwrap();
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines: Vec<_> = stdout.lines().collect();
        assert_eq!(lines.first().copied(), Some("Placed R1 and U1, connected POWER, moved R1"));
        assert!(lines.contains(&"Undid and redid the route"));
        assert_eq!(lines.last().copied(), Some(REOPENED));
        assert!(!lines.iter().any(|line| line.starts_with("Analysis")));

        // Advanced Analysis And Cancellation Use The Same Caller Directory.
        let output = Command::new(&cli).current_dir(directory.path()).arg(backend).arg(&plugin).arg("--advanced").output().unwrap();
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines = stdout.lines().map(str::to_owned);
        advanced_outputs.push(lines.collect::<Vec<_>>());
    }

    // Only The Native Backend Lists The Caller's Directories.
    let guest = advanced_outputs.pop().unwrap();
    let native = advanced_outputs.pop().unwrap();
    let (directories, shared): (Vec<_>, Vec<_>) = native.into_iter().partition(|line| line.starts_with("Native directories:"));
    assert_eq!(directories, [r#"Native directories: ["alpha", "zeta"]"#]);
    assert_eq!(shared, guest);
    for expected in ["Analysis settled: 3 findings", "Cancelled analysis and waited for settlement", REOPENED] {
        assert!(guest.iter().any(|line| line == expected), "missing {expected}");
    }
}
