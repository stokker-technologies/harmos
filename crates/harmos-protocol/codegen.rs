/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Authoritative Protocol Bindings.

/* === Modules === */

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

pub fn generate(
    compiler: &std::path::Path, includes: &std::path::Path, output: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let mut config = prost_build::Config::new();
    config.protoc_executable(compiler);
    config.file_descriptor_set_path(output.join("descriptors.bin"));
    tonic_prost_build::configure()
        .client_mod_attribute("harmos.v1", "#[cfg(not(target_arch = \"wasm32\"))]")
        .server_mod_attribute("harmos.v1", "#[cfg(not(target_arch = \"wasm32\"))]")
        .build_client(true)
        .build_server(true)
        .out_dir(output)
        .compile_with_config(
            config,
            &[proto.join("harmos/v1/plugin.proto"), proto.join("harmos/v1/host.proto")],
            &[proto, includes.into()],
        )?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
