/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Harmos CLI Stories: a product exports the protocols, checks its contracts and generates every SDK language.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

// Third Party Imports
use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorProto, FileDescriptorSet};
use tempfile::TempDir;

// Local Imports

/* === Constants === */

const LIBRARY: &str = "../harmos-protocol/src/descriptors.bin";

const APPLICATION: &str = "../../sdk/rust/tests/fixtures/client/client-descriptors.bin";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn source(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn harmos(arguments: &[&str]) -> Output {
    // Archived Runs Extract the Binary Away From Its Build Path, So Nextest Names the Remapped One.
    let binary = env::var_os("NEXTEST_BIN_EXE_harmos").unwrap_or_else(|| env!("CARGO_BIN_EXE_harmos").into());
    Command::new(binary).args(arguments).output().expect("the harmos binary runs")
}

fn read(folder: &Path, relative: &str) -> String {
    fs::read_to_string(folder.join(relative)).unwrap_or_else(|error| panic!("{relative}: {error}"))
}

fn protocols(folder: &Path, root: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(folder).expect("protocol folder") {
        let path = entry.expect("protocol entry").path();
        if path.is_dir() {
            protocols(&path, root, found);
        } else if path.extension().is_some_and(|extension| extension == "proto") {
            found.push(path.strip_prefix(root).expect("below the root").to_path_buf());
        }
    }
}

/* === Tests === */

#[test]
fn exported_protocols_match_the_authoritative_sources() {
    // Export Into an Empty Folder
    let out = TempDir::new().unwrap();
    let exported = harmos(&["proto", "export", "--out", out.path().to_str().unwrap()]);
    assert!(exported.status.success(), "{}", String::from_utf8_lossy(&exported.stderr));

    // Compare Every Authoritative File
    let root = source("../../proto");
    let mut expected = Vec::new();
    protocols(&root.join("harmos"), &root, &mut expected);
    let mut actual = Vec::new();
    protocols(&out.path().join("harmos"), out.path(), &mut actual);
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected);
    for relative in &expected {
        assert_eq!(fs::read(out.path().join(relative)).unwrap(), fs::read(root.join(relative)).unwrap(), "{}", relative.display());
    }
}

#[test]
fn protocol_checks_count_callers_and_refuse_conflicts() {
    // Accept the Library Contracts
    let library = source(LIBRARY);
    let checked = harmos(&["proto", "check", "--descriptors", library.to_str().unwrap()]);
    assert!(checked.status.success());
    assert!(String::from_utf8_lossy(&checked.stdout).starts_with("Protocol Callers: "));

    // Refuse Calls Whose Callers Declaration Is Gone
    let folder = TempDir::new().unwrap();
    let stripped = folder.path().join("stripped.pb");
    let set = FileDescriptorSet::decode(&fs::read(&library).unwrap()[..]).unwrap();
    fs::write(&stripped, set.encode_to_vec()).unwrap();
    let refused = harmos(&["proto", "check", "--descriptors", stripped.to_str().unwrap()]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("Requires CLIENTS, PLUGINS or ALL"));

    // Refuse Two Definitions of One File
    let mut paths = Vec::new();
    for message in ["First", "Second"] {
        let file = FileDescriptorProto {
            name: Some("example/model/v1/values.proto".into()),
            package: Some("example.model.v1".into()),
            message_type: vec![DescriptorProto {
                name: Some(message.into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let path = folder.path().join(format!("{message}.pb"));
        fs::write(&path, FileDescriptorSet { file: vec![file] }.encode_to_vec()).unwrap();
        paths.push(path);
    }
    let out = folder.path().join("out");
    let refused = harmos(&[
        "codegen",
        "rust",
        "--descriptors",
        paths[0].to_str().unwrap(),
        "--descriptors",
        paths[1].to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("Conflicting Descriptor File example/model/v1/values.proto"));
}

#[test]
fn product_bindings_follow_their_explicit_options() {
    let library = source(LIBRARY);
    let application = source(APPLICATION);
    let inputs = ["--namespace", "application", "--descriptors", library.to_str().unwrap(), "--descriptors", application.to_str().unwrap()];
    let out = TempDir::new().unwrap();
    let render = |language: &str, options: &[&str]| {
        let folder = out.path().join(language);
        let mut arguments = vec!["codegen", language, "--out", folder.to_str().unwrap()];
        arguments.extend(inputs);
        arguments.extend(options);
        let rendered = harmos(&arguments);
        assert!(rendered.status.success(), "{language}: {}", String::from_utf8_lossy(&rendered.stderr));
        folder
    };

    // Rust: Selectors, Product Bindings and Crate-Private Host Calls
    let rust = render(
        "rust",
        &[
            "--selector",
            "application.host.v1.jobs.ProgressWatchRequest=job",
            "--host-visibility",
            "crate",
            "--exclude-service",
            "harmos.plugin.v1.Time",
        ],
    );
    let host = read(&rust, "plugin/host.rs");
    assert!(host.contains("pub(crate) fn host(&self) -> HostRoot {"));
    assert!(host.contains("    TimeGet,") && !host.contains("HostTimeResource"));
    assert!(read(&rust, "plugin/host_exports.rs").contains("pub(crate) use self::host::{"));
    assert!(read(&rust, "host.rs").contains("impl Request for harmos_protocol::host::InvocationsGetRequest"));
    assert!(read(&rust, "client/resources.rs").contains("pub async fn start(&self"));

    // Python: Protocol Modules Follow the Root Mapping
    let python = render("python", &["--module", "application=app._generated.proto"]);
    let client = read(&python, "client.py");
    assert!(client.contains("from app._generated.proto.host.v1.jobs_pb2 import JobsStartRequest"));
    assert!(read(&python, "host.py").contains("from harmos._generated.proto.model.v1.operation_pb2 import VISIBILITY_PUBLIC"));

    // C#: Renamed and Internal Projections
    let csharp = render("csharp", &["--csharp-namespace", "Product", "--internal-host", "--rename-type", "JobsStartResponse=JobStarted"]);
    assert!(read(&csharp, "Host.g.cs").contains("internal sealed class HostRoot(Context context)"));
    assert!(read(&csharp, "Client.g.cs").contains("JobStarted"));

    // C++: Transport Root and Product Error Hook
    let cpp = render("cpp", &["--client-transport", "--rpc-error", "::application::detail::rpc_error"]);
    let transport = read(&cpp, "include/application/_generated/client.hpp");
    assert!(transport.contains("struct Transport"));
    assert!(transport.contains("::application::detail::rpc_error("));
}
