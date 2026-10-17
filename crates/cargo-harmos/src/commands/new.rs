/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Copy A Complete Starter With An Explicit Harmos Dependency Source.

/* === Modules === */
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/templates.rs"));
}


/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::{Path, PathBuf};

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use harmos_packaging::Error;


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Args)]
pub struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Args)]
struct Project {
    destination: PathBuf,
    #[arg(long, conflicts_with = "harmos_path", required_unless_present = "harmos_path")]
    harmos_rev: Option<String>,
    #[arg(long, conflicts_with = "harmos_rev", required_unless_present = "harmos_rev")]
    harmos_path: Option<PathBuf>,
}


/* === Enums === */
#[derive(clap::Subcommand)]
enum Command {
    #[command(about = "Create A Native Sidecar Plugin.")]
    Sidecar(Project),
    #[command(about = "Create A Sandboxed Guest Plugin.")]
    Guest(Project),
}


/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn run(arguments: Arguments) -> Result<(), Error> {
    let (project, kind, files) = match arguments.command {
        Command::Sidecar(project) => (project, "sidecar", embedded::SIDECAR),
        Command::Guest(project) => (project, "guest", embedded::GUEST),
    };
    let package = project.destination.file_name().and_then(|name| name.to_str()).ok_or_else(|| Error::Settings("destination must name a package".into()))?;
    if package.is_empty() || !package.starts_with(|c: char| c.is_ascii_lowercase()) || !package.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')) {
        return Err(Error::Settings("package must be lowercase letters, numbers, hyphens or underscores".into()));
    }
    let module = package.replace('-', "_");
    if ["self", "super", "crate", "mod", "type", "fn", "impl", "enum", "struct", "use", "pub", "async", "await", "move", "match", "loop", "return", "ref", "static", "const", "trait", "where", "let", "in", "for", "while", "if", "else", "true", "false", "extern", "unsafe", "dyn", "mut", "as", "break", "continue", "abstract", "become", "box", "do", "final", "gen", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield", "union", "_"].contains(&module.as_str()) {
        return Err(Error::Settings("package cannot be a Rust keyword".into()));
    }
    let title = package.split(['-', '_']).map(|part| {
        let mut chars = part.chars();
        chars.next().map(|first| first.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ");
    if let Some(revision) = &project.harmos_rev {
        if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(Error::Settings("--harmos-rev requires the full published Git revision".into()));
        }
    }
    let local = project.harmos_path.as_ref().map(std::fs::canonicalize).transpose()?;
    if let Some(root) = &local {
        for name in ["harmos-domain", &format!("harmos-{kind}")] {
            if !root.join("crates").join(name).join("Cargo.toml").is_file() {
                return Err(Error::Settings("--harmos-path must name a Harmos source checkout".into()));
            }
        }
    }
    let mut prepared = Vec::new();
    for (name, bytes) in files {
        let original = std::str::from_utf8(bytes).map_err(|_| Error::Settings("template is not UTF-8".into()))?;
        let mut text = original.replace("__PACKAGE_NAME__", package).replace("__RUST_MODULE__", &module).replace("__PRODUCT_NAME__", &title);
        if *name == "Cargo.toml" {
            if let Some(root) = &local {
                let mut lines = Vec::new();
                for line in text.lines() {
                    let mut line = line.to_owned();
                    for dependency in ["harmos-domain".to_owned(), format!("harmos-{kind}")] {
                        if line.starts_with(&format!("{dependency} =")) {
                            let path = root.join("crates").join(&dependency);
                            let replacement = format!("path = {}", serde_json::to_string(&path.to_string_lossy())?);
                            line = line.replace("git = \"https://github.com/stokker-technologies/harmos\", rev = \"__HARMOS_REVISION__\"", &replacement);
                        }
                    }
                    lines.push(line);
                }
                text = lines.join("\n") + "\n";
            }
        }
        text = text.replace("__HARMOS_REVISION__", project.harmos_rev.as_deref().unwrap_or(""));
        prepared.push((*name, finalize_marker(&text)));
    }
    let parent = project.destination.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    // create_dir Reserves A New Destination And Refuses Existing User Content.
    std::fs::create_dir(&project.destination)?;
    for (name, content) in prepared {
        let path = project.destination.join(name);
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        std::fs::write(path, content)?;
    }
    println!("{}", project.destination.display());
    Ok(())
}


/* === Private Functions === */
// Rebind Only The Rendered Body Digest After Final Substitution.
fn finalize_marker(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let mut start = 0usize;
    if text.starts_with("#!") { start = text.find('\n').map_or(text.len(), |end| end + 1); }
    if text.starts_with("---\n") {
        if let Some(end) = text[4..].find("\n---\n") { start = 4 + end + 5; }
    }
    let end = text[start..].find('\n').map_or(text.len(), |offset| start + offset + 1);
    let line = &text[start..end];
    if !line.contains("Rendered by stkr project update from ") { return text; }
    let Some((prefix, _)) = line.split_once("; sha256:") else { return text };
    let mut body = text.clone();
    body.replace_range(start..end, "");
    let digest = format!("{:x}", Sha256::digest(body.as_bytes()));
    let closing = if line.trim_end().ends_with("-->") { " -->" } else { "" };
    format!("{}{prefix}; sha256:{digest}{closing}\n{}", &text[..start], &text[end..])
}


/* === Tests === */
