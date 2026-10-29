/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Embed The Authored And Hub-Rendered Starter Assets.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or("missing package root")?);
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing build output")?);
    let mut source = String::new();
    for kind in ["sidecar", "guest"] {
        let directory = root.join("templates").join(kind);
        source.push_str(&embed(&directory, kind)?);
    }
    std::fs::write(output.join("templates.rs"), source)?;
    Ok(())
}

fn embed(directory: &Path, kind: &str) -> Result<String, Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut paths = Vec::new();
    collect(directory, directory, &mut paths)?;
    paths.sort();

    let mut source = String::new();
    writeln!(source, "pub const {}: &[(&str, &[u8])] = &[", kind.to_uppercase())?;
    for relative in paths {
        let file = relative.to_str().ok_or("template name is not UTF-8")?.replace('\\', "/");
        let name = file.strip_suffix(".tmpl").ok_or("template source must end in .tmpl")?;
        let path = directory.join(&relative);
        writeln!(source, "({name:?}, include_bytes!({:?})),", path.to_str().ok_or("template path is not UTF-8")?)?;
    }
    source.push_str("];\n");
    Ok(source)
}

fn collect(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        collect_entry(root, entry?, output)?;
    }
    Ok(())
}

fn collect_entry(root: &Path, entry: std::fs::DirEntry, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let kind = entry.file_type()?;
    if kind.is_symlink() {
        return Err(std::io::Error::other("template sources cannot be symbolic links"));
    }
    let path = entry.path();
    if kind.is_dir() {
        return collect(root, &path, output);
    }
    if !kind.is_file() {
        return Ok(());
    }
    let relative = path.strip_prefix(root).map_err(std::io::Error::other)?;
    output.push(relative.to_owned());
    Ok(())
}
