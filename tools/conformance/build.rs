/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Caller Admission Fixture Contracts

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{env, error::Error, path::PathBuf};

// Third Party Imports
use prost_build::Config;

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
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("Generate Caller Probe: Missing Manifest Directory")?);
    let fixture = root.join("../../sdk/tests/fixtures/conformance/proto");
    let library = root.join("../../proto");
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("Generate Caller Probe: Missing Output Directory")?);
    let mut config = Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    config.skip_source_info();
    config.extern_path(".harmos.model.v1", "::harmos_protocol::model");
    config.file_descriptor_set_path(output.join("caller-descriptors.bin"));
    config.out_dir(output);
    config.compile_protos(
        &[fixture.join("conformance/host/v1/caller_probe.proto")],
        &[fixture.clone(), library.clone(), protoc_bin_vendored::include_path()?],
    )?;
    println!("cargo:rerun-if-changed={}", fixture.join("conformance/host/v1/caller_probe.proto").display());
    println!("cargo:rerun-if-changed={}", library.join("harmos/model/v1/callers.proto").display());
    Ok(())
}

/* === Tests === */
