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
    codegen::generate(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (source, target) in
        [("schematic.v1.rs", "messages.rs"), ("schematic.reflection.rs", "reflection.rs"), ("schematic.descriptors.bin", "descriptors.bin")]
    {
        fs::copy(output.path().join(source), destination.join(target))?;
    }
    Ok(())
}

/* === Tests === */
