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
use sha2::{Digest, Sha256};

// Local Imports
use harmos_packaging::{Build, Error};
use harmos_packaging::config::{BuildBackend, BuildProfile, BuildSettings};


/* === Constants === */
const PRODUCER: &str = "path+file:///plugin#schematic-sidecar@0.1.0";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Captured {
    messages: String,
    metadata: String,
    receipt: Value,
    settings: BuildSettings,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Captured {
    fn new(messages: &[Value], selected: BTreeMap<String, String>) -> Self {
        let artifacts = messages.iter().filter(|message| message["reason"] == "compiler-artifact" && message["profile"]["test"] == false)
            .filter_map(|message| message["executable"].as_str())
            .map(|path| (path.to_owned(), digest(&std::fs::read(path).unwrap())))
            .collect::<BTreeMap<_, _>>();
        let target = artifacts.keys().next().map(|path| Path::new(path).parent().unwrap().parent().unwrap().to_owned())
            .unwrap_or_else(|| std::env::temp_dir().join("unused-cargo-target"));
        let packages = selected.iter().map(|(id, name)| json!({"id": id, "name": name,
            "manifest_path": target.join(name).join("Cargo.toml"), "source": null})).collect::<Vec<_>>();
        let members = selected.keys().collect::<Vec<_>>();
        let metadata = json!({"packages": packages, "workspace_members": members,
            "workspace_default_members": members, "workspace_root": target, "target_directory": target,
            "resolve": {"root": null, "nodes": []}}).to_string();
        let messages = messages.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let receipt = json!({"schema": 1, "profile": "dev", "target": "x86_64-unknown-linux-gnu",
            "command": ["cargo", "build", "--locked"],
            "inputs": {"rustc": "host: x86_64-unknown-linux-gnu"},
            "messages": {"sha256": digest(messages.as_bytes())},
            "metadata": {"sha256": digest(metadata.as_bytes())},
            "artifacts": artifacts, "generated": {}});
        Self { messages, metadata, receipt, settings: BuildSettings {
            backend: BuildBackend::Sidecar, profile: BuildProfile::Dev,
            target: Some("x86_64-unknown-linux-gnu".into()), packages: selected.into_values().collect(),
            ..Default::default()
        }}
    }

    fn collect(self) -> Result<Build, Error> {
        Build::from_cargo_outputs(Cursor::new(self.messages), Cursor::new(self.metadata),
            Cursor::new(self.receipt.to_string()), &self.settings)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn artifact(path: &Path, test: bool) -> Value {
    json!({"reason": "compiler-artifact", "package_id": PRODUCER,
        "target": {"name": "schematic-sidecar", "kind": ["bin"]},
        "profile": {"test": test}, "executable": path, "filenames": [path]})
}

fn collect(messages: &[Value], selected: BTreeMap<String, String>) -> Result<Build, Error> {
    Captured::new(messages, selected).collect()
}

fn selected() -> BTreeMap<String, String> {
    BTreeMap::from([(PRODUCER.into(), "schematic-sidecar".into())])
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("debug");
    std::fs::create_dir(&output).unwrap();
    let path = output.join("schematic-sidecar");
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
    let second = directory.path().join("debug/different-sidecar");
    std::fs::write(&second, b"another emitted output").unwrap();
    let stream = [artifact(&path, false), artifact(&second, false), json!({"reason": "build-finished", "success": true})];
    assert!(matches!(collect(&stream, selected()), Err(Error::Settings(_))));
}

#[test]
fn mismatched_backend_target_is_refused() {
    let mut capture = Captured::new(&[], selected());
    capture.settings.target = Some("wasm32-unknown-unknown".into());
    capture.receipt["target"] = json!("wasm32-unknown-unknown");
    let result = capture.collect();
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

#[test]
fn collection_refuses_bytes_changed_after_compile() {
    let (_directory, path) = fixture();
    let capture = Captured::new(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], selected());
    std::fs::write(&path, b"not the producer output").unwrap();
    assert!(matches!(capture.collect(), Err(Error::ArtifactChanged(changed)) if changed == path));
}

#[test]
fn collection_refuses_changed_messages() {
    let (_directory, path) = fixture();
    let mut capture = Captured::new(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], selected());
    capture.messages.push(' ');
    assert!(matches!(capture.collect(), Err(Error::Settings(_))));
}

#[test]
fn collection_refuses_changed_metadata() {
    let (_directory, path) = fixture();
    let mut capture = Captured::new(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], selected());
    capture.metadata.push(' ');
    assert!(matches!(capture.collect(), Err(Error::Settings(_))));
}

#[test]
fn collection_refuses_requested_profile_relabel() {
    let (_directory, path) = fixture();
    let mut capture = Captured::new(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], selected());
    capture.settings.profile = BuildProfile::Release;
    assert!(matches!(capture.collect(), Err(Error::Settings(_))));
}

#[test]
fn collection_refuses_schema_changed_after_compile() {
    let (directory, path) = fixture();
    let output = directory.path().join("debug/build/plugin/out");
    let catalog = output.join("grpc/operations");
    std::fs::create_dir_all(&catalog).unwrap();
    let descriptor = catalog.join("descriptors.bin");
    let manifest = catalog.join("manifest.txt");
    std::fs::write(&descriptor, b"original descriptor bytes").unwrap();
    std::fs::write(&manifest, b"descriptors.bin\n").unwrap();
    let messages = [artifact(&path, false), json!({"reason": "build-script-executed", "package_id": PRODUCER, "out_dir": output}),
        json!({"reason": "build-finished", "success": true})];
    let mut capture = Captured::new(&messages, selected());
    let generated = [&descriptor, &manifest].into_iter().map(|path| (path.to_str().unwrap().to_owned(), digest(&std::fs::read(path).unwrap())))
        .collect::<BTreeMap<_, _>>();
    capture.receipt["generated"] = json!(generated);
    std::fs::write(&descriptor, b"replaced after compile").unwrap();
    assert!(matches!(capture.collect(), Err(Error::ArtifactChanged(changed)) if changed == descriptor));
}
