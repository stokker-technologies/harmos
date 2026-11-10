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

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{fs, path::Path};

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

/* === Tests === */

#[test]
fn generated_code_matches_proto() -> Result<(), Box<dyn std::error::Error>> {
    // Locate the Wire Definition and Disposable Output.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let proto = root.join("proto");
    let output_root = root.join("target/generated/rust/harmos-protocol");
    fs::create_dir_all(&output_root)?;
    let output = tempfile::tempdir_in(output_root)?;

    // Regenerate With the Pinned Compiler, Without Mutating the Environment.
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    tonic_prost_build::configure().build_client(true).build_server(true).out_dir(output.path()).compile_with_config(
        config,
        &[proto.join("harmos/v1/plugin.proto")],
        &[proto, protoc_bin_vendored::include_path()?],
    )?;

    // Compare the Complete Generated Source, Retaining Evidence on Failure.
    let generated = fs::read(output.path().join("harmos.v1.rs"))?;
    let committed = fs::read(root.join("crates/harmos-protocol/src/v1.rs"))?;
    if generated != committed {
        let output = output.keep();
        panic!("generated Rust differs from src/v1.rs; regenerated source retained in {}", output.display());
    }
    Ok(())
}
