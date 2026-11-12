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
    for (source, target) in [
        ("harmos.v1.rs", "v1.rs"),
        ("harmos.context.v1.rs", "context.rs"),
        ("harmos.plugin.v1.rs", "plugin.rs"),
        ("harmos.storage.v1.rs", "storage.rs"),
        ("reflection.rs", "reflection.rs"),
        ("descriptors.bin", "descriptors.bin"),
        ("guest-descriptors.bin", "guest-descriptors.bin"),
    ] {
        std::fs::copy(output.path().join(source), destination.join(target))?;
    }
    codegen::generate_example(&protoc_bin_vendored::protoc_bin_path()?, &protoc_bin_vendored::include_path()?, output.path())?;
    let destination = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/schematic/protocol/src");
    std::fs::create_dir_all(&destination)?;
    for (source, target) in
        [("schematic.v1.rs", "messages.rs"), ("schematic.reflection.rs", "reflection.rs"), ("schematic.descriptors.bin", "descriptors.bin")]
    {
        std::fs::copy(output.path().join(source), destination.join(target))?;
    }
    Ok(())
}

/* === Tests === */
