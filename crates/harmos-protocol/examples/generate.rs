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
use std::{error::Error, fs, path::Path};

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

fn main() -> Result<(), Box<dyn Error>> {
    let output = tempfile::tempdir()?;
    codegen::generate(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in fs::read_dir(output.path())? {
        let path = entry?.path();
        let name = path.file_name().ok_or("missing generated name")?.to_string_lossy();
        if name.starts_with("harmos.") && name.ends_with(".rs") {
            let module = codegen::module_path(name.trim_end_matches(".rs"))?;
            let target = destination.join(format!("{}.rs", module.replace("::", "/")));
            fs::create_dir_all(target.parent().ok_or("missing generated parent")?)?;
            fs::copy(path, target)?;
        }
    }
    for name in ["reflection.rs", "descriptors.bin", "guest-descriptors.bin"] {
        fs::copy(output.path().join(name), destination.join(name))?;
    }
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/src/context/types");
    fs::create_dir_all(&destination)?;
    for entry in fs::read_dir(&destination)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "bin") {
            fs::remove_file(path)?;
        }
    }
    for entry in fs::read_dir(output.path().join("context-types"))? {
        let path = entry?.path();
        fs::copy(&path, destination.join(path.file_name().ok_or("missing context type filename")?))?;
    }
    codegen::generate_client_fixture(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/tests/fixtures/client");
    fs::create_dir_all(&destination)?;
    for name in ["application.v1.rs", "application.v1.jobs.rs", "client-descriptors.bin", "client-reflection.rs"] {
        fs::copy(output.path().join(name), destination.join(name))?;
    }
    codegen::generate_conformance(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/rust/tests/fixtures/conformance");
    fs::create_dir_all(&destination)?;
    for (source, target) in [("harmos.conformance.v1.rs", "generated.rs"), ("conformance-descriptors.bin", "descriptors.bin")] {
        fs::copy(output.path().join(source), destination.join(target))?;
    }
    Ok(())
}

/* === Tests === */
