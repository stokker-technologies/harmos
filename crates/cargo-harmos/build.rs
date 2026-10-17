/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Embed The Authored And Hub-Rendered Starter Assets.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt::Write as _;
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or("missing package root")?);
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing build output")?);
    let mut source = String::new();
    for kind in ["sidecar", "guest"] {
        let directory = root.join("templates").join(kind);
        println!("cargo:rerun-if-changed={}", directory.display());
        let mut paths = Vec::new();
        collect(&directory, &directory, &mut paths)?;
        paths.sort();
        writeln!(
            source,
            "pub const {}: &[(&str, &[u8])] = &[",
            kind.to_uppercase()
        )?;
        for relative in paths {
            let name = relative
                .to_str()
                .ok_or("template name is not UTF-8")?
                .replace('\\', "/");
            let path = directory.join(&relative);
            writeln!(
                source,
                "({name:?}, include_bytes!({:?})),",
                path.to_str().ok_or("template path is not UTF-8")?
            )?;
        }
        source.push_str("];\n");
    }
    std::fs::write(output.join("templates.rs"), source)?;
    Ok(())
}

fn collect(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(std::io::Error::other(
                "template sources cannot be symbolic links",
            ));
        }
        if kind.is_dir() {
            collect(root, &entry.path(), output)?;
        }
        if kind.is_file() {
            output.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(std::io::Error::other)?
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/* === Tests === */
