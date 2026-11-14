/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Walk the CLI Across Both Built Plugin Backends

/* === Imports === */

// Standard Library Imports
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::{env, fs, path::PathBuf};

// Third Party Imports
use serde_json::{Value, json};

// Local Imports
use super::support::fixtures;

/* === Constants === */

/* === Enums === */

#[derive(Debug, PartialEq, Eq)]
enum Startup {
    Initializing,
    Ready,
}

/* === Tests === */

#[test]
fn cli_backends_reopen() {
    let _report = crate::report::system();
    let directory = tempfile::tempdir().unwrap();
    let reports = tempfile::tempdir().unwrap();
    let report_directory = env::var_os("STKR_TEST_REPORT_DIR").map(PathBuf::from).unwrap_or_else(|| reports.path().to_owned());
    let cli = fixtures::artifact("schematic-cli");
    let mut children = Vec::new();

    for (backend, artifact) in [("--sidecar", "schematic-sidecar"), ("--guest", "schematic-guest")] {
        let plugin = fixtures::artifact(artifact);

        // Nominal Editing Persists Through The Backend And Reopens.
        let child = fixtures::command(&cli)
            .env("STKR_TEST_REPORT_DIR", &report_directory)
            .current_dir(directory.path())
            .arg(backend)
            .arg(&plugin)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        children.push((backend, child.map(|child| (child.id(), child))));
    }

    let mut outputs = Vec::new();
    for (backend, child) in children {
        let output = child.and_then(|(process, child)| Child::wait_with_output(child).map(|output| (process, output)));
        outputs.push((backend, output));
    }
    for (backend, output) in outputs {
        let (process, output) = output.unwrap_or_else(|error| panic!("{backend}: {error}"));
        assert!(
            output.status.success(),
            "{backend}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(report_directory.join(format!("schematic-walkthrough-{process}.json"))).unwrap();
        let report: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["document"]["symbols"].as_object().unwrap().len(), 2);
        assert_eq!(report["document"]["nets"].as_object().unwrap().len(), 1);
        assert_eq!(report["document"]["nets"]["1"]["route"].as_array().unwrap().len(), 3);
        assert!(report["analysis"].is_null());
    }
}

#[test]
fn cli_streams_settle() {
    let _report = crate::report::system();
    let directory = tempfile::tempdir().unwrap();
    let reports = tempfile::tempdir().unwrap();
    let report_directory = env::var_os("STKR_TEST_REPORT_DIR").map(PathBuf::from).unwrap_or_else(|| reports.path().to_owned());
    std::fs::create_dir(directory.path().join("zeta")).unwrap();
    std::fs::create_dir(directory.path().join("alpha")).unwrap();
    std::fs::write(directory.path().join("file.txt"), "not a directory").unwrap();
    let cli = fixtures::artifact("schematic-cli");
    let mut children = Vec::new();

    for (backend, artifact) in [("--sidecar", "schematic-sidecar"), ("--guest", "schematic-guest")] {
        let plugin = fixtures::artifact(artifact);

        // Start The Next Backend Only After This CLI Has Initialized Its Plugin.
        let mut child = fixtures::command(&cli)
            .env("STKR_TEST_REPORT_DIR", &report_directory)
            .current_dir(directory.path())
            .arg(backend)
            .arg(&plugin)
            .arg("--advanced")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("{backend}: {error}"));
        let process = child.id();
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
        let readiness =
            ready.recv_timeout(<schematic_operations::AnalyzeSchematic as harmos_runtime::operations::ProtobufOperation>::POLICY.timeout);
        if initializing != Ok(Startup::Initializing) || readiness != Ok(Startup::Ready) {
            let _ = child.kill();
            let output = child.wait_with_output().expect("the failed CLI is reaped");
            let printed = reader.join().expect("the output reader settles").map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
            panic!(
                "{backend} did not announce readiness within operation policy: {initializing:?}, {readiness:?}\n{printed:?}\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        children.push((backend, process, child, reader));
    }

    let mut outputs = Vec::new();
    for (backend, process, child, reader) in children {
        let mut output = child.wait_with_output().unwrap_or_else(|error| panic!("{backend}: {error}"));
        output.stderr = reader.join().expect("the startup reader settles").expect("the CLI startup events are readable");
        outputs.push((backend, process, output));
    }
    let mut advanced_outputs = Vec::new();
    for (backend, process, output) in outputs {
        assert!(
            output.status.success(),
            "{backend}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(report_directory.join(format!("schematic-walkthrough-{process}.json"))).unwrap();
        let report: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["document"]["symbols"].as_object().unwrap().len(), 2);
        assert_eq!(report["document"]["nets"].as_object().unwrap().len(), 1);
        assert_eq!(report["document"]["nets"]["1"]["route"].as_array().unwrap().len(), 3);
        assert_eq!(report["analysis"]["checked"], 3);
        assert_eq!(report["analysis"]["findings"], 3);
        assert_eq!(report["analysis"]["cancelled"], true);
        advanced_outputs.push(report);
    }

    // Only The Native Backend Lists The Caller's Directories.
    let guest = advanced_outputs.pop().unwrap();
    let mut native = advanced_outputs.pop().unwrap();
    assert_eq!(native["analysis"]["directories"], json!(["alpha", "zeta"]));
    assert!(guest["analysis"]["directories"].is_null());
    native["analysis"]["directories"] = Value::Null;
    assert_eq!(native, guest);
}
