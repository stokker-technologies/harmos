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
use harmos_packaging::config::{BuildBackend, BuildProfile, BuildSettings};
use harmos_packaging::{Build, Error};

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

enum ReceiptSurface {
    Messages,
    Metadata,
    Profile,
}

/* === Traits === */

/* === Implementations === */

impl Captured {
    fn new(messages: &[Value], selected: BTreeMap<String, String>) -> Self {
        let artifacts = messages
            .iter()
            .filter(|message| message["reason"] == "compiler-artifact" && message["profile"]["test"] == false)
            .filter_map(|message| message["executable"].as_str())
            .map(|path| (path.to_owned(), digest(&std::fs::read(path).unwrap())))
            .collect::<BTreeMap<_, _>>();
        let target = artifacts
            .keys()
            .next()
            .map(|path| Path::new(path).parent().unwrap().parent().unwrap().to_owned())
            .unwrap_or_else(|| std::env::temp_dir().join("unused-cargo-target"));
        let packages = selected
            .iter()
            .map(|(id, name)| {
                json!({"id": id, "name": name,
            "manifest_path": target.join(name).join("Cargo.toml"), "source": null})
            })
            .collect::<Vec<_>>();
        let members = selected.keys().collect::<Vec<_>>();
        let metadata = json!({"packages": packages, "workspace_members": members,
            "workspace_default_members": members, "workspace_root": target, "target_directory": target,
            "resolve": {"root": null, "nodes": []}})
        .to_string();
        let messages = messages.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let receipt = json!({"schema": 1, "profile": "dev", "target": "x86_64-unknown-linux-gnu",
            "command": ["cargo", "build", "--locked"],
            "inputs": {"rustc": "host: x86_64-unknown-linux-gnu"},
            "messages": {"sha256": digest(messages.as_bytes())},
            "metadata": {"sha256": digest(metadata.as_bytes())},
            "artifacts": artifacts, "generated": {}});
        Self {
            messages,
            metadata,
            receipt,
            settings: BuildSettings {
                backend: BuildBackend::Sidecar,
                profile: BuildProfile::Dev,
                target: Some("x86_64-unknown-linux-gnu".into()),
                packages: selected.into_values().collect(),
                ..Default::default()
            },
        }
    }

    fn collect(self) -> Result<Build, Error> {
        Build::from_cargo_outputs(
            Cursor::new(self.messages),
            Cursor::new(self.metadata),
            Cursor::new(self.receipt.to_string()),
            &self.settings,
        )
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

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
fn test_harnesses_excluded() {
    let (_directory, path) = fixture();
    let stream =
        [artifact(Path::new("absent-test-harness"), true), artifact(&path, false), json!({"reason": "build-finished", "success": true})];
    let build = collect(&stream, selected()).unwrap();
    assert_eq!(build.artifacts.len(), 1);
    assert_eq!(build.artifacts[0].path, path);
    assert_eq!(build.profile, BuildProfile::Dev);
    assert!(!build.artifacts[0].sha256.is_empty());
}

#[test]
fn test_profile_keeps_identity() {
    let (_directory, path) = fixture();
    let messages = [artifact(&path, false), json!({"reason": "build-finished", "success": true})];
    let profile: BuildProfile = "test".parse().unwrap();
    assert_eq!(profile.name(), "test");
    let mut capture = Captured::new(&messages, selected());
    capture.settings.profile = profile;
    capture.receipt["profile"] = json!("test");
    capture.receipt["command"] = json!(["cargo", "build", "--locked", "--profile", "test"]);
    let build = capture.collect().unwrap();
    assert_eq!(build.profile, profile);
    assert_eq!(build.artifacts[0].path, path);

    let mut mismatched = Captured::new(&messages, selected());
    mismatched.receipt["profile"] = json!("test");
    assert!(matches!(mismatched.collect(), Err(Error::Settings(_))));
}

#[test]
fn completion_refuses_invalid() {
    let (_directory, path) = fixture();
    let streams = [
        ("incomplete", vec![artifact(&path, false)]),
        ("failed", vec![artifact(&path, false), json!({"reason": "build-finished", "success": false})]),
        (
            "post-completion",
            vec![json!({"reason": "build-finished", "success": true}), json!({"reason": "build-finished", "success": true})],
        ),
    ];
    for (name, stream) in streams {
        assert!(matches!(collect(&stream, selected()), Err(Error::Cargo(_))), "{name}");
    }
}

#[test]
fn selection_refuses_conflicts() {
    {
        let (_directory, path) = fixture();
        let mut packages = selected();
        packages.insert("path+file:///missing#other@0.1.0".into(), "other".into());
        assert!(matches!(
            collect(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], packages),
            Err(Error::Settings(_))
        ));
    }
    {
        let (directory, path) = fixture();
        let second = directory.path().join("debug/different-sidecar");
        std::fs::write(&second, b"another emitted output").unwrap();
        let stream = [artifact(&path, false), artifact(&second, false), json!({"reason": "build-finished", "success": true})];
        assert!(matches!(collect(&stream, selected()), Err(Error::Settings(_))));
    }
    {
        let mut capture = Captured::new(&[], selected());
        capture.settings.target = Some("wasm32-unknown-unknown".into());
        capture.receipt["target"] = json!("wasm32-unknown-unknown");
        let result = capture.collect();
        assert!(matches!(result, Err(Error::Settings(_))));
    }
}

#[test]
fn snapshot_guards_bytes() {
    let (_directory, path) = fixture();
    let original = std::fs::read(&path).unwrap();
    let messages = [artifact(&path, false), json!({"reason": "build-finished", "success": true})];
    let collection = Captured::new(&messages, selected());
    let packaging = Captured::new(&messages, selected());
    std::fs::write(&path, b"not the producer output").unwrap();
    assert!(matches!(collection.collect(), Err(Error::ArtifactChanged(changed)) if changed == path));

    std::fs::write(&path, original).unwrap();
    let build = packaging.collect().unwrap();
    std::fs::write(&path, b"replaced after the build").unwrap();
    let output = tempfile::tempdir().unwrap();
    assert!(matches!(harmos_packaging::package(&build, output.path()), Err(Error::ArtifactChanged(changed)) if changed == path));
    assert_eq!(std::fs::read_dir(output.path()).unwrap().count(), 0);
}

#[test]
fn receipt_refuses_tampering() {
    for surface in [ReceiptSurface::Messages, ReceiptSurface::Metadata, ReceiptSurface::Profile] {
        let (_directory, path) = fixture();
        let mut capture = Captured::new(&[artifact(&path, false), json!({"reason": "build-finished", "success": true})], selected());
        match surface {
            ReceiptSurface::Messages => capture.messages.push(' '),
            ReceiptSurface::Metadata => capture.metadata.push(' '),
            ReceiptSurface::Profile => capture.settings.profile = BuildProfile::Release,
        }
        assert!(matches!(capture.collect(), Err(Error::Settings(_))));
    }
}

#[test]
fn collection_checks_schema() {
    let (directory, path) = fixture();
    let output = directory.path().join("debug/build/plugin/out");
    let catalog = output.join("grpc/operations");
    std::fs::create_dir_all(&catalog).unwrap();
    let descriptor = catalog.join("descriptors.bin");
    let manifest = catalog.join("manifest.txt");
    std::fs::write(&descriptor, b"original descriptor bytes").unwrap();
    std::fs::write(&manifest, b"descriptors.bin\n").unwrap();
    let messages = [
        artifact(&path, false),
        json!({"reason": "build-script-executed", "package_id": PRODUCER, "out_dir": output}),
        json!({"reason": "build-finished", "success": true}),
    ];
    let mut capture = Captured::new(&messages, selected());
    let generated = [&descriptor, &manifest]
        .into_iter()
        .map(|path| (path.to_str().unwrap().to_owned(), digest(&std::fs::read(path).unwrap())))
        .collect::<BTreeMap<_, _>>();
    capture.receipt["generated"] = json!(generated);
    std::fs::write(&descriptor, b"replaced after compile").unwrap();
    // A Generated File Replaced After Compilation No Longer Matches The Producer Snapshot.
    assert!(matches!(capture.collect(), Err(Error::Settings(reason)) if reason.contains("generated schema set differs")));
}
