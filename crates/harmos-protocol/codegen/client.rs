/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Generate Independent Product Contracts for SDK Composition.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{collections::BTreeMap, error::Error, fs, path::Path};

// Third Party Imports
use prost::Message;
use prost_build::Config;
use prost_types::FileDescriptorSet;

// Local Imports
use super::reflection;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Generate Independent Library and Product Contracts for SDK Composition Stories.
pub fn generate_client_fixture(compiler: &Path, includes: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let proto = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/tests/fixtures/client/proto");
    let mut files = Vec::new();
    let mut folders = vec![proto.clone()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|extension| extension == "proto") {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut config = Config::new();
    config.protoc_executable(compiler);
    config.extern_path(".harmos.model.v1", "::harmos_protocol::model");
    config.extern_path(".harmos.host.v1", "::harmos_protocol::host");
    config.file_descriptor_set_path(output.join("client-descriptors.bin"));
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    tonic_prost_build::configure().out_dir(output).compile_with_config(config, &files, &[proto, library, includes.into()])?;
    let descriptors: FileDescriptorSet = FileDescriptorSet::decode(fs::read(output.join("client-descriptors.bin"))?.as_slice())?;
    let mut modules = BTreeMap::new();
    for file in &descriptors.file {
        let package = file.package.as_deref().ok_or("fixture package missing")?;
        let Some(suffix) = package.strip_prefix("application.host.v1") else {
            continue;
        };
        let suffix = suffix.trim_start_matches('.');
        let module = if suffix.is_empty() { "crate::host".to_owned() } else { format!("crate::host::{}", suffix.replace('.', "::")) };
        modules.insert(package.to_owned(), module);
    }
    let source = reflection::generate(&descriptors, &modules, "client-descriptors.bin")?;
    fs::write(output.join("client-reflection.rs"), source)?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
