/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Regenerate Schematic Protocol Bindings

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
    codegen::generate(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(".."),
        &protoc_bin_vendored::protoc_bin_path()?,
        &protoc_bin_vendored::include_path()?,
        output.path(),
    )?;
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (source, target) in [
        ("schematic.host.v1.rs", "host.rs"),
        ("schematic.model.v1.rs", "model.rs"),
        ("schematic.plugin.v1.rs", "plugin.rs"),
        ("schematic.reflection.rs", "reflection.rs"),
        ("schematic.descriptors.bin", "descriptors.bin"),
        ("schematic.client.rs", "client.rs"),
    ] {
        fs::copy(output.path().join(source), destination.join(target))?;
    }
    Ok(())
}

/* === Tests === */
