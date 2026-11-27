/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Check the Committed Bindings Against the Wire Definition.

/* === Modules === */

mod audience_faces;
mod schema_fingerprints;
mod shared_primitives;
mod support;

#[cfg(unix)]
mod unix;

#[cfg(target_os = "windows")]
mod windows;

/* === Re-Exports === */

#[cfg(unix)]
pub(crate) use unix::prepare_compiler;

#[cfg(target_os = "windows")]
pub(crate) use windows::prepare_compiler;

/* === Imports === */

// Standard Library Imports
use std::env::consts::{ARCH, OS};
use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

// Third Party Imports

// Local Imports
use crate::codegen;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

// The Versioned Vendor Folder Holds Exactly One Locked Package per Prefix.
fn vendored(vendor: &Path, prefix: &str) -> Result<PathBuf, Box<dyn Error>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(vendor)? {
        let path = entry?.path();
        if path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(prefix)) {
            found.push(path);
        }
    }
    match <[PathBuf; 1]>::try_from(found) {
        Ok([package]) => Ok(package),
        Err(found) => Err(format!("Locate Vendored {prefix}*: Expected One Locked Package, Found {}", found.len()).into()),
    }
}

/* === Tests === */

#[test]
fn generated_code_matches_proto() -> Result<(), Box<dyn Error>> {
    // Locate the Wire Definition and Disposable Output.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_root = root.join("target/generated/rust/harmos-protocol");
    fs::create_dir_all(&output_root)?;
    let output = tempfile::tempdir_in(output_root)?;

    // Archived Tests Consume the Transferred Compiler and Sources, Not the Producer's Registry.
    let (compiler, includes, prost_types) = if let Ok(profile) = env::var("STKR_TEST_PROFILE") {
        let target = format!("{}-{}", OS, ARCH);
        let vendor = root.join("target/build").join(profile).join(target).join("fixtures/inputs/vendor");
        let platform = match (OS, ARCH) {
            ("linux", "x86_64") => "linux-x86_64",
            ("linux", "aarch64") => "linux-aarch_64",
            ("macos", "x86_64") => "macos-x86_64",
            ("macos", "aarch64") => "macos-aarch_64",
            ("windows", _) => "win32",
            other => panic!("no vendored protoc for {other:?}"),
        };
        let package = vendored(&vendor, &format!("protoc-bin-vendored-{platform}-"))?;
        let executable = if OS == "windows" { "protoc.exe" } else { "protoc" };
        (package.join("bin").join(executable), package.join("include"), vendored(&vendor, "prost-types-")?)
    } else {
        (protoc_bin_vendored::protoc_bin_path()?, protoc_bin_vendored::include_path()?, codegen::locked_prost_types()?)
    };
    prepare_compiler(&compiler)?;

    // Regenerate With the Pinned Compiler, Without Mutating the Environment.
    codegen::generate(&compiler, &includes, &prost_types, output.path())?;

    // Compare the Complete Generated Source Without Dumping Descriptor Bytes.
    for entry in fs::read_dir(output.path())? {
        let path = entry?.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("harmos.") || !name.ends_with(".rs") {
            continue;
        }
        let module = codegen::module_path(name.trim_end_matches(".rs"))?;
        let committed = root.join(format!("crates/harmos-protocol/src/{}.rs", module.replace("::", "/")));
        assert!(fs::read(path)? == fs::read(committed)?, "Generated Module Differs: {module}; Run mise run generate -- --language rust");
    }
    for (name, committed) in [
        ("reflection.rs", include_bytes!("../src/reflection.rs").as_slice()),
        ("descriptors.bin", include_bytes!("../src/descriptors.bin").as_slice()),
        ("guest-descriptors.bin", include_bytes!("../src/guest-descriptors.bin").as_slice()),
        ("clock-descriptors.bin", include_bytes!("../src/clock-descriptors.bin").as_slice()),
        ("message-options-descriptors.bin", include_bytes!("../src/message-options-descriptors.bin").as_slice()),
    ] {
        assert!(fs::read(output.path().join(name))?.as_slice() == committed, "Generated Artifact Differs: {name}; Regenerate the Rust SDK");
    }
    for entry in fs::read_dir(output.path().join("context-types"))? {
        let path = entry?.path();
        let committed = root.join("sdk/rust/src/context/types").join(path.file_name().unwrap());
        assert!(fs::read(&path)? == fs::read(committed)?, "Generated Context Types Differ: {}; Regenerate the Rust SDK", path.display());
    }
    codegen::generate_client_fixture(&compiler, &includes, output.path())?;
    for name in [
        "application.host.v1.rs",
        "application.host.v1.jobs.rs",
        "application.host.v1.plugins.rs",
        "client-descriptors.bin",
        "client-reflection.rs",
    ] {
        assert!(
            fs::read(output.path().join(name))? == fs::read(root.join("sdk/rust/tests/fixtures/client").join(name))?,
            "Generated Client Fixture Differs: {name}; Regenerate the Rust SDK"
        );
    }
    codegen::generate_conformance(&compiler, &includes, output.path())?;
    for (source, target) in [
        ("conformance.model.v1.rs", "generated.rs"),
        ("conformance-descriptors.bin", "descriptors.bin"),
        ("caller-request.bin", "caller-request.bin"),
    ] {
        assert!(
            fs::read(output.path().join(source))? == fs::read(root.join("sdk/rust/tests/fixtures/conformance").join(target))?,
            "Generated Conformance Fixture Differs: {source}; Regenerate the Rust SDK"
        );
    }
    Ok(())
}
