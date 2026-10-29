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
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};

// Third Party Imports

// Local Imports
use super::support::fixtures;

/* === Constants === */

const REOPENED: &str = "Reopened: 2 symbols, 1 net, 3 route points";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Debug, PartialEq, Eq)]
enum Startup {
    Initializing,
    Ready,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn cli_backends_reopen() {
    let _report = crate::report::system();
    let directory = tempfile::tempdir().unwrap();
    let cli = fixtures::artifact("schematic-cli");
    let mut children = Vec::new();

    for (backend, artifact) in [("--sidecar", "schematic-sidecar"), ("--guest", "schematic-guest")] {
        let plugin = fixtures::artifact(artifact);

        // Nominal Editing Persists Through The Backend And Reopens.
        let child = fixtures::command(&cli)
            .current_dir(directory.path())
            .arg(backend)
            .arg(&plugin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        children.push((backend, child));
    }

    let mut outputs = Vec::new();
    for (backend, child) in children {
        let output = child.and_then(Child::wait_with_output);
        outputs.push((backend, output));
    }
    for (backend, output) in outputs {
        let output = output.unwrap_or_else(|error| panic!("{backend}: {error}"));
        assert!(
            output.status.success(),
            "{backend}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines: Vec<_> = stdout.lines().collect();
        assert_eq!(lines.first().copied(), Some("Placed R1 and U1, connected POWER, moved R1"));
        assert!(lines.contains(&"Undid and redid the route"));
        assert_eq!(lines.last().copied(), Some(REOPENED));
        assert!(!lines.iter().any(|line| line.starts_with("Analysis")));
    }
}

#[test]
fn cli_streams_settle() {
    let _report = crate::report::system();
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("zeta")).unwrap();
    std::fs::create_dir(directory.path().join("alpha")).unwrap();
    std::fs::write(directory.path().join("file.txt"), "not a directory").unwrap();
    let cli = fixtures::artifact("schematic-cli");
    let mut children = Vec::new();

    for (backend, artifact) in [("--sidecar", "schematic-sidecar"), ("--guest", "schematic-guest")] {
        let plugin = fixtures::artifact(artifact);

        // Start The Next Backend Only After This CLI Has Initialized Its Plugin.
        let mut child = fixtures::command(&cli)
            .current_dir(directory.path())
            .arg(backend)
            .arg(&plugin)
            .arg("--advanced")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("{backend}: {error}"));
        let stderr = child.stderr.take().expect("the CLI startup events are piped");
        let (started, ready) = std::sync::mpsc::sync_channel(2);
        let reader = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut stderr = BufReader::new(stderr);
            let mut bytes = Vec::new();
            loop {
                let start = bytes.len();
                if stderr.read_until(b'\n', &mut bytes)? == 0 {
                    break;
                }
                let event = match bytes[start..].trim_ascii() {
                    b"schematic.plugin.initializing" => Startup::Initializing,
                    b"schematic.plugin.ready" => Startup::Ready,
                    _ => continue,
                };
                let _ = started.send(event);
            }
            Ok(bytes)
        });
        // Wasm Compilation Is Preparation, Outside The Operation Deadline.
        // The System Story Policy Still Bounds The Whole Walkthrough.
        let initializing = ready.recv();
        let readiness = ready.recv_timeout(schematic_operations::POLICY.timeout);
        if initializing != Ok(Startup::Initializing) || readiness != Ok(Startup::Ready) {
            let _ = child.kill();
            let output = child.wait_with_output().expect("the failed CLI is reaped");
            let printed = reader.join().expect("the output reader settles").map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
            panic!(
                "{backend} did not announce readiness within operation policy: {initializing:?}, {readiness:?}\n{printed:?}\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        children.push((backend, child, reader));
    }

    let mut outputs = Vec::new();
    for (backend, child, reader) in children {
        let mut output = child.wait_with_output().unwrap_or_else(|error| panic!("{backend}: {error}"));
        output.stderr = reader.join().expect("the startup reader settles").expect("the CLI startup events are readable");
        outputs.push((backend, output));
    }
    let mut advanced_outputs = Vec::new();
    for (backend, output) in outputs {
        assert!(
            output.status.success(),
            "{backend}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let lines = stdout.lines().map(str::to_owned);
        advanced_outputs.push(lines.collect::<Vec<_>>());
    }

    // Only The Native Backend Lists The Caller's Directories.
    let guest = advanced_outputs.pop().unwrap();
    let native = advanced_outputs.pop().unwrap();
    let is_directory = |line: &String| line.starts_with("Native directories:");
    let (directories, shared): (Vec<_>, Vec<_>) = native.into_iter().partition(is_directory);
    assert_eq!(directories, [r#"Native directories: ["alpha", "zeta"]"#]);
    assert_eq!(shared, guest);
    for expected in ["Analysis settled: 3 findings", "Cancelled analysis and waited for settlement", REOPENED] {
        assert!(guest.iter().any(|line| line == expected), "missing {expected}");
    }
}
