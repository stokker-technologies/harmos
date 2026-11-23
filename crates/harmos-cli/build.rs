/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bundle the Authoritative Protocol Sources Into the Binary.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join("../../proto");
    println!("cargo:rerun-if-changed={}", root.join("harmos").display());

    // Collect Protocol Files in Path Order
    let mut files = Vec::new();
    collect(&root.join("harmos"), &mut files)?;
    files.sort();

    // Render the Bundled Inventory
    let mut source = String::from("pub(crate) const PROTOCOLS: &[(&str, &[u8])] = &[\n");
    for path in files {
        let relative = path.strip_prefix(&root).expect("collected below the protocol root").to_string_lossy().replace('\\', "/");
        let absolute = path.display().to_string();
        source.push_str(&format!("    ({relative:?}, include_bytes!({absolute:?})),\n"));
    }
    source.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("protocols.rs");
    fs::write(output, source)
}

fn collect(folder: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, files)?;
            continue;
        }
        if path.extension().is_some_and(|extension| extension == "proto") {
            files.push(path);
        }
    }
    Ok(())
}

/* === Tests === */
