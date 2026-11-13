/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Regenerate Checked-In Protocol Bindings.

/* === Modules === */

#[path = "../codegen.rs"]
mod codegen;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = tempfile::tempdir()?;
    codegen::generate(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(output.path())? {
        let path = entry?.path();
        let name = path.file_name().ok_or("missing generated name")?.to_string_lossy();
        if name.starts_with("harmos.") && name.ends_with(".rs") {
            let module = codegen::module_path(name.trim_end_matches(".rs"))?;
            let target = destination.join(format!("{}.rs", module.replace("::", "/")));
            std::fs::create_dir_all(target.parent().ok_or("missing generated parent")?)?;
            std::fs::copy(path, target)?;
        }
    }
    for name in ["reflection.rs", "descriptors.bin", "guest-descriptors.bin"] {
        std::fs::copy(output.path().join(name), destination.join(name))?;
    }
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/src/context/types");
    std::fs::create_dir_all(&destination)?;
    for entry in std::fs::read_dir(&destination)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "bin") {
            std::fs::remove_file(path)?;
        }
    }
    for entry in std::fs::read_dir(output.path().join("context-types"))? {
        let path = entry?.path();
        std::fs::copy(&path, destination.join(path.file_name().ok_or("missing context type filename")?))?;
    }
    codegen::generate_example(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/schematic/protocol/src");
    std::fs::create_dir_all(&destination)?;
    for (source, target) in
        [("schematic.v1.rs", "messages.rs"), ("schematic.reflection.rs", "reflection.rs"), ("schematic.descriptors.bin", "descriptors.bin")]
    {
        std::fs::copy(output.path().join(source), destination.join(target))?;
    }
    codegen::generate_client_fixture(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/tests/fixtures/client");
    std::fs::create_dir_all(&destination)?;
    for name in ["application.v1.rs", "application.v1.jobs.rs", "client-descriptors.bin", "client-reflection.rs"] {
        std::fs::copy(output.path().join(name), destination.join(name))?;
    }
    codegen::generate_conformance(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/tests/fixtures/conformance");
    std::fs::create_dir_all(&destination)?;
    for (source, target) in [("harmos.conformance.v1.rs", "generated.rs"), ("conformance-descriptors.bin", "descriptors.bin")] {
        std::fs::copy(output.path().join(source), destination.join(target))?;
    }
    Ok(())
}

/* === Tests === */
