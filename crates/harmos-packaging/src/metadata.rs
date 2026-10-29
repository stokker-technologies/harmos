/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Resolve The Selected Cargo Graph And Its Local Build Inputs Once.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use super::{BuildCancellation, Error, process::Process};
use crate::config::BuildSettings;

#[derive(Deserialize)]
pub(super) struct Metadata {
    pub(super) packages: Vec<Package>,
    pub(super) workspace_members: Vec<String>,
    pub(super) workspace_default_members: Vec<String>,
    pub(super) workspace_root: PathBuf,
    pub(super) target_directory: PathBuf,
    pub(super) resolve: Resolve,
}

#[derive(Deserialize)]
pub(super) struct Package {
    pub(super) id: String,
    pub(super) name: String,
    manifest_path: PathBuf,
    source: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Resolve {
    root: Option<String>,
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    dependencies: Vec<String>,
}

#[derive(Clone)]
pub(super) struct Inputs {
    pub(super) roots: BTreeSet<PathBuf>,
    pub(super) files: BTreeSet<PathBuf>,
    pub(super) excluded: BTreeSet<PathBuf>,
}

impl Metadata {
    pub(super) fn read(settings: &BuildSettings, cancellation: &BuildCancellation) -> Result<Self, Error> {
        let manifest = settings.manifest.canonicalize()?;
        let directory = manifest.parent().ok_or_else(|| Error::Settings("manifest has no project directory".into()))?;
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command.current_dir(directory).args(["metadata", "--locked", "--format-version=1", "--manifest-path"]).arg(&manifest);
        if !settings.default_features {
            command.arg("--no-default-features");
        }
        if !settings.features.is_empty() {
            command.args(["--features", &settings.features.join(",")]);
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = Process::spawn(command, cancellation)?.output()?;
        cancellation.check()?;
        if !output.status.success() {
            return Err(Error::Cargo(String::from_utf8_lossy(&output.stderr).into_owned()));
        }
        Ok(serde_json::from_slice(&output.stdout)?)
    }

    pub(super) fn select(&self, settings: &BuildSettings) -> Result<BTreeMap<String, String>, Error> {
        let members: BTreeSet<_> = self.workspace_members.iter().collect();
        let defaults: BTreeSet<_> =
            self.resolve.root.iter().chain(self.workspace_default_members.iter().filter(|_| self.resolve.root.is_none())).collect();
        let selected: BTreeMap<_, _> = self
            .packages
            .iter()
            .filter(|package| {
                members.contains(&package.id)
                    && if settings.packages.is_empty() { defaults.contains(&package.id) } else { settings.packages.contains(&package.name) }
            })
            .map(|package| (package.id.clone(), package.name.clone()))
            .collect();
        if selected.is_empty() || settings.packages.iter().any(|name| !selected.values().any(|selected| selected == name)) {
            return Err(Error::Settings("packages must select workspace members".into()));
        }
        Ok(selected)
    }

    pub(super) fn inputs(&self, settings: &BuildSettings) -> Result<Inputs, Error> {
        let selected = self.select(settings)?;
        let graph: BTreeMap<_, _> = self.resolve.nodes.iter().map(|node| (&node.id, &node.dependencies)).collect();
        let mut pending: Vec<_> = selected.keys().cloned().collect();
        let mut dependencies = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if dependencies.insert(id.clone())
                && let Some(children) = graph.get(&id)
            {
                pending.extend(children.iter().cloned());
            }
        }
        let roots = self
            .packages
            .iter()
            .filter(|package| package.source.is_none() && dependencies.contains(&package.id))
            .map(|package| package.manifest_path.parent().expect("Cargo package manifest has a parent").to_owned())
            .collect();
        let mut inputs = Inputs {
            roots,
            files: BTreeSet::new(),
            excluded: BTreeSet::from([self.target_directory.clone(), self.output_directory(settings)?]),
        };
        inputs.files.insert(self.workspace_root.join("Cargo.toml"));
        inputs.files.insert(self.workspace_root.join("Cargo.lock"));
        let manifest = settings.manifest.canonicalize()?;
        for parent in manifest.parent().expect("manifest has a parent").ancestors() {
            inputs.files.insert(parent.join(".cargo/config"));
            inputs.files.insert(parent.join(".cargo/config.toml"));
        }
        Ok(inputs)
    }

    pub(super) fn output_directory(&self, settings: &BuildSettings) -> Result<PathBuf, Error> {
        let manifest = settings.manifest.canonicalize()?;
        let directory = manifest.parent().expect("manifest has a parent");
        Ok(settings
            .target_directory
            .as_ref()
            .map(|path| if path.is_absolute() { path.clone() } else { directory.join(path) })
            .unwrap_or_else(|| self.target_directory.clone()))
    }
}

impl Inputs {
    pub(super) fn fallback(settings: &BuildSettings) -> Result<Self, Error> {
        let manifest = settings.manifest.canonicalize()?;
        let directory = manifest.parent().expect("manifest has a parent").to_owned();
        let target = settings
            .target_directory
            .clone()
            .or_else(|| std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from))
            .unwrap_or_else(|| directory.join("target"));
        let target = if target.is_absolute() { target } else { directory.join(target) };
        Ok(Self {
            roots: BTreeSet::from([directory]),
            files: BTreeSet::from([manifest]),
            excluded: BTreeSet::from([target]),
        })
    }

    pub(super) fn relevant(&self, path: &Path) -> bool {
        if self.excluded.iter().any(|root| path.starts_with(root) || staging_sibling(root, path)) {
            return false;
        }
        if self.files.contains(path) || self.files.iter().any(|file| file.starts_with(path)) {
            return true;
        }
        self.roots.iter().any(|root| {
            path.strip_prefix(root).is_ok_and(|relative| {
                !relative.components().any(|component| {
                    matches!(component.as_os_str().to_str(), Some("target" | "dist" | ".git" | ".stkr" | ".venv" | "node_modules"))
                })
            })
        })
    }
}

// Cargo Creates A Missing Output Directory Atomically Through A Sibling Named After It Plus Six Random Characters
fn staging_sibling(root: &Path, path: &Path) -> bool {
    let (Some(parent), Some(name)) = (root.parent(), root.file_name().and_then(|name| name.to_str())) else {
        return false;
    };
    let Some(first) = path.strip_prefix(parent).ok().and_then(|relative| relative.components().next()) else {
        return false;
    };
    first
        .as_os_str()
        .to_str()
        .and_then(|first| first.strip_prefix(name))
        .is_some_and(|suffix| suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_inputs_excluded() {
        let root = PathBuf::from("workspace");
        let output = root.join("generated");
        let inputs = Inputs {
            roots: BTreeSet::from([root.clone(), PathBuf::from("dependency")]),
            files: BTreeSet::from([output.join("schema.rs"), root.join("Cargo.toml")]),
            excluded: BTreeSet::from([output.clone()]),
        };
        assert!(!inputs.relevant(&output.join("schema.rs")));
        assert!(!inputs.relevant(&output.join("nested/new.rs")));
        for directory in ["target", "dist", ".git", ".stkr", ".venv", "node_modules"] {
            assert!(!inputs.relevant(&root.join(directory).join("output")));
        }
        assert!(inputs.relevant(&root.join("src/model.rs")));
        assert!(inputs.relevant(&root.join("Cargo.toml")));
        assert!(inputs.relevant(&PathBuf::from("dependency/src/lib.rs")));
        assert!(!inputs.relevant(&PathBuf::from("unrelated/src/lib.rs")));
    }

    #[test]
    fn atomic_outputs_ignored() {
        let root = PathBuf::from("project");
        let inputs = Inputs {
            roots: BTreeSet::from([root.clone()]),
            files: BTreeSet::from([root.join("Cargo.toml")]),
            excluded: BTreeSet::from([root.join("build")]),
        };
        assert!(!inputs.relevant(&root.join("buildyQTpmE")));
        assert!(!inputs.relevant(&root.join("buildyQTpmE/debug")));
        assert!(inputs.relevant(&root.join("build_notes.rs")));
        assert!(inputs.relevant(&root.join("build_script")));
    }
}
