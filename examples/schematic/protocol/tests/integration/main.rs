/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Check Schematic Protocol Generation

/* === Modules === */

#[path = "../../codegen.rs"]
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

/* === Tests === */

#[test]
fn generated_code_matches_proto() -> Result<(), Box<dyn Error>> {
    let output = tempfile::tempdir()?;
    codegen::generate(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(".."),
        &protoc_bin_vendored::protoc_bin_path()?,
        &protoc_bin_vendored::include_path()?,
        output.path(),
    )?;
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for (source, target) in
        [("schematic.v1.rs", "messages.rs"), ("schematic.reflection.rs", "reflection.rs"), ("schematic.descriptors.bin", "descriptors.bin")]
    {
        assert_eq!(fs::read(output.path().join(source))?, fs::read(destination.join(target))?, "generated {target} drifted");
    }
    Ok(())
}
