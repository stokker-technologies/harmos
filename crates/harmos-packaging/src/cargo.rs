/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Cargo output collection shared by application authoring tools.

use crate::Error;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct CompilerArtifact {
    pub producer: String,
    pub name: String,
    pub path: PathBuf,
    pub symbols: Option<PathBuf>,
    binary: bool,
}

#[derive(Debug)]
pub struct CargoOutputs {
    pub artifacts: Vec<CompilerArtifact>,
    pub schema_directories: BTreeSet<(String, PathBuf)>,
}

impl CargoOutputs {
    /// Select exactly one native executable per requested binary, retaining its
    /// emitted symbols path without reading or changing the compiler outputs.
    pub fn executables(self, producer: &str, names: &[String]) -> Result<BTreeMap<String, (PathBuf, Option<PathBuf>)>, Error> {
        let requested: BTreeSet<_> = names.iter().collect();
        if requested.is_empty() || requested.len() != names.len() {
            return Err(Error::Settings("select distinct native binary names".into()));
        }
        let mut selected: BTreeMap<String, Vec<(PathBuf, Option<PathBuf>)>> = BTreeMap::new();
        for artifact in self.artifacts {
            if artifact.producer == producer && artifact.binary && requested.contains(&artifact.name) {
                selected.entry(artifact.name).or_default().push((artifact.path, artifact.symbols));
            }
        }
        for paths in selected.values_mut() {
            paths.sort();
            paths.dedup();
        }
        if selected.len() != requested.len() || selected.values().any(|paths| paths.len() != 1) {
            return Err(Error::Settings("Cargo must report exactly one successful executable for each selected binary".into()));
        }
        Ok(selected.into_iter().map(|(name, mut paths)| (name, paths.remove(0))).collect())
    }
}

#[derive(Deserialize)]
struct Message {
    reason: String,
    package_id: Option<String>,
    executable: Option<PathBuf>,

    #[serde(default)]
    filenames: Vec<PathBuf>,

    target: Option<Target>,
    out_dir: Option<PathBuf>,
    message: Option<Diagnostic>,
    success: Option<bool>,
    profile: Option<Profile>,
}

#[derive(Deserialize)]
struct Diagnostic {
    rendered: Option<String>,
}

#[derive(Deserialize)]
struct Profile {
    test: bool,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
}

pub fn collect(reader: impl BufRead, selected: &BTreeMap<String, String>) -> Result<CargoOutputs, Error> {
    collect_with_diagnostics(reader, selected, |_| {})
}

pub(crate) fn collect_with_diagnostics(
    reader: impl BufRead, selected: &BTreeMap<String, String>, mut diagnostic: impl FnMut(&str),
) -> Result<CargoOutputs, Error> {
    let mut artifacts = Vec::new();
    let mut outputs = BTreeSet::new();
    let mut finished = false;
    for line in reader.lines() {
        let message: Message = serde_json::from_str(&line?)?;
        if finished {
            return Err(Error::Cargo("messages follow the completed Cargo build".into()));
        }
        if message.reason == "build-finished" {
            if message.success != Some(true) {
                return Err(Error::Cargo("captured Cargo build did not succeed".into()));
            }
            finished = true;
            continue;
        }
        if let Some(rendered) = message.message.as_ref().and_then(|diagnostic| diagnostic.rendered.as_ref()) {
            diagnostic(rendered);
        }
        let Some(producer) = message.package_id.as_ref().and_then(|id| selected.get(id)) else {
            continue;
        };
        if message.reason == "build-script-executed" {
            if let Some(directory) = message.out_dir {
                outputs.insert((producer.clone(), directory));
            }
            continue;
        }
        if message.reason != "compiler-artifact" || message.profile.is_some_and(|profile| profile.test) {
            continue;
        }
        let Some(target) = message.target else {
            continue;
        };
        if target.kind.iter().any(|kind| kind == "custom-build" || kind == "example" || kind == "test" || kind == "bench") {
            continue;
        }
        let symbols = message.filenames.iter().find(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pdb"))).cloned();
        let files = message
            .executable
            .into_iter()
            .chain(message.filenames.into_iter().filter(|path| path.extension().is_some_and(|extension| extension == "wasm")));
        for path in files {
            artifacts.push(CompilerArtifact {
                producer: producer.clone(),
                name: target.name.clone(),
                path,
                symbols: symbols.clone(),
                binary: target.kind.iter().any(|kind| kind == "bin"),
            });
        }
    }
    if !finished {
        return Err(Error::Cargo("captured Cargo stream has no successful build-finished event".into()));
    }
    artifacts.sort_by(|left, right| (&left.producer, &left.name, &left.path).cmp(&(&right.producer, &right.name, &right.path)));
    artifacts.dedup_by(|left, right| left.producer == right.producer && left.name == right.name && left.path == right.path);
    Ok(CargoOutputs { artifacts, schema_directories: outputs })
}

/* === Tests === */
