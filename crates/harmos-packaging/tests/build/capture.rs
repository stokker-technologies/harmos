/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Refuse Incomplete Or Ambiguous Cargo Streams Before Packaging.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

// Third Party Imports
use serde_json::{Value, json};

// Local Imports
use harmos_packaging::{Build, Error};
use harmos_packaging::config::{BuildBackend, BuildProfile};


/* === Constants === */
const PRODUCER: &str = "path+file:///plugin#schematic-sidecar@0.1.0";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn artifact(path: &Path, test: bool) -> Value {
    json!({"reason": "compiler-artifact", "package_id": PRODUCER,
        "target": {"name": "schematic-sidecar", "kind": ["bin"]},
        "profile": {"test": test}, "executable": path, "filenames": [path]})
}

fn collect(messages: &[Value], selected: BTreeMap<String, String>) -> Result<Build, Error> {
    let stream = messages.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    Build::from_cargo_messages(Cursor::new(stream), BuildBackend::Sidecar,
        BuildProfile::Dev, "x86_64-unknown-linux-gnu".into(), &selected)
}

fn selected() -> BTreeMap<String, String> {
    BTreeMap::from([(PRODUCER.into(), "schematic-sidecar".into())])
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("schematic-sidecar");
    std::fs::write(&path, b"compiler-emitted bytes").unwrap();
    (directory, path)
}


/* === Tests === */

#[test]
fn completed_stream_excludes_test_harnesses() {
    let (_directory, path) = fixture();
    let stream = [artifact(Path::new("absent-test-harness"), true), artifact(&path, false),
        json!({"reason": "build-finished", "success": true})];
    let build = collect(&stream, selected()).unwrap();
    assert_eq!(build.artifacts.len(), 1);
    assert_eq!(build.artifacts[0].path, path);
    assert_eq!(build.profile, BuildProfile::Dev);
    assert!(!build.artifacts[0].sha256.is_empty());
}

#[test]
fn incomplete_stream_is_refused() {
    let (_directory, path) = fixture();
    assert!(matches!(collect(&[artifact(&path, false)], selected()), Err(Error::Cargo(_))));
}

#[test]
fn failed_build_is_refused() {
    let (_directory, path) = fixture();
    assert!(matches!(collect(&[artifact(&path, false), json!({"reason": "build-finished", "success": false})], selected()), Err(Error::Cargo(_))));
}

#[test]
fn missing_selected_producer_is_refused() {
    let (_directory, path) = fixture();
    let mut packages = selected();
    packages.insert("path+file:///missing#other@0.1.0".into(), "other".into());
    assert!(matches!(collect(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], packages), Err(Error::Settings(_))));
}

#[test]
fn ambiguous_selected_outputs_are_refused() {
    let (directory, path) = fixture();
    let second = directory.path().join("different-sidecar");
    std::fs::write(&second, b"another emitted output").unwrap();
    let stream = [artifact(&path, false), artifact(&second, false), json!({"reason": "build-finished", "success": true})];
    assert!(matches!(collect(&stream, selected()), Err(Error::Settings(_))));
}

#[test]
fn mismatched_backend_target_is_refused() {
    let result = Build::from_cargo_messages(Cursor::new(""), BuildBackend::Guest,
        BuildProfile::Dev, "x86_64-unknown-linux-gnu".into(), &selected());
    assert!(matches!(result, Err(Error::Settings(_))));
}

#[test]
fn messages_after_completion_are_refused() {
    let stream = [json!({"reason": "build-finished", "success": true}), json!({"reason": "build-finished", "success": true})];
    assert!(matches!(collect(&stream, selected()), Err(Error::Cargo(_))));
}

#[test]
fn packaging_refuses_changed_artifact_bytes() {
    let (_directory, path) = fixture();
    let build = collect(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], selected()).unwrap();
    std::fs::write(&path, b"replaced after the build").unwrap();
    let output = tempfile::tempdir().unwrap();
    assert!(matches!(harmos_packaging::package(&build, output.path()), Err(Error::ArtifactChanged(changed)) if changed == path));
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
}
