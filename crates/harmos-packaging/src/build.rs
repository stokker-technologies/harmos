/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Select Artifacts And Schema Outputs From One Successful Cargo Invocation.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Third Party Imports
use crate::digest;
use serde::Deserialize;

// Local Imports
use super::{BuildCancellation, Error, metadata::Metadata, process::Process};
use crate::config::{BuildBackend, BuildProfile, BuildSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type BuildMessages = (Vec<Artifact>, BTreeSet<(String, PathBuf)>);

/* === Structs === */

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub producer: String,
    pub name: String,
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub producer: String,
    pub catalog: String,
    pub directory: PathBuf,
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub backend: BuildBackend,
    pub profile: BuildProfile,
    pub target: String,
    pub artifacts: Vec<Artifact>,
    pub schemas: Vec<Schema>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageOutput {
    pub producer: String,
    pub path: PathBuf,
    pub sha256: String,
}

// A Narrow Projection Of The Generic Archive Owner's Versioned Receipt.
#[derive(Deserialize)]
struct Capture {
    schema: u32,
    profile: BuildProfile,
    target: String,
    command: Vec<String>,
    inputs: CaptureInputs,
    messages: CaptureFile,
    metadata: CaptureFile,
    artifacts: BTreeMap<PathBuf, String>,
    generated: BTreeMap<PathBuf, String>,
}

#[derive(Deserialize)]
struct CaptureInputs {
    rustc: String,
}

#[derive(Deserialize)]
struct CaptureFile {
    sha256: String,
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Artifact {
    // Stamp An Existing Cargo Output Without Launching A Compiler.
    pub fn read(producer: String, name: String, path: PathBuf) -> Result<Self, Error> {
        safe_name(&producer)?;
        safe_name(&name)?;
        let sha256 = digest(&read_artifact(&path)?);
        Ok(Self { producer, name, path, sha256 })
    }
}

impl Schema {
    pub fn read(producer: String, catalog: String, directory: PathBuf) -> Result<Self, Error> {
        safe_name(&producer)?;
        safe_name(&catalog)?;
        let files = schema_files(&directory)?.into_iter().map(|(name, bytes)| (name, digest(&bytes))).collect();
        Ok(Self { producer, catalog, directory, files })
    }
}

impl Build {
    // The Archive Producer Owns Invocation Identity; This Collector Binds Its
    // Captured Streams And Checks Output Locations Without Running A Compiler.
    pub fn from_cargo_outputs(
        mut messages: impl BufRead, mut metadata: impl std::io::Read, receipt: impl std::io::Read, settings: &BuildSettings,
    ) -> Result<Self, Error> {
        validate_backend(settings)?;
        let receipt: Capture = serde_json::from_reader(receipt)?;
        if receipt.schema != 1 || receipt.command.is_empty() || receipt.target.is_empty() {
            return Err(Error::Settings("unsupported or incomplete archive producer receipt".into()));
        }
        if receipt.profile != settings.profile || settings.target.as_ref().is_some_and(|target| target != &receipt.target) {
            return Err(Error::Settings("requested profile or target disagrees with archive producer receipt".into()));
        }
        let mut message_bytes = Vec::new();
        messages.read_to_end(&mut message_bytes)?;
        let mut metadata_bytes = Vec::new();
        metadata.read_to_end(&mut metadata_bytes)?;
        if digest(&message_bytes) != receipt.messages.sha256 || digest(&metadata_bytes) != receipt.metadata.sha256 {
            return Err(Error::Settings("captured Cargo streams disagree with archive producer byte stamps".into()));
        }
        let metadata: Metadata = serde_json::from_slice(&metadata_bytes)?;
        let packages = metadata.select(settings)?;
        if packages.is_empty() || packages.values().collect::<BTreeSet<_>>().len() != packages.len() {
            return Err(Error::Settings("collection needs unambiguous selected Cargo packages".into()));
        }
        if (settings.backend == BuildBackend::Guest) != receipt.target.starts_with("wasm32-") {
            return Err(Error::Settings("backend and producer target disagree".into()));
        }
        let (artifacts, outputs) = read_messages(std::io::Cursor::new(message_bytes), &packages)?;
        let profile_directory = match receipt.profile {
            BuildProfile::Dev | BuildProfile::Test => "debug",
            BuildProfile::Release | BuildProfile::Bench => "release",
        };
        let native = receipt.inputs.rustc.lines().find_map(|line| line.strip_prefix("host: "));
        let expected = metadata.target_directory.join(&receipt.target).join(profile_directory);
        let host = metadata.target_directory.join(profile_directory);
        let matches = |path: &Path| path.starts_with(&expected) || (native == Some(receipt.target.as_str()) && path.starts_with(&host));
        if artifacts.iter().any(|artifact| !matches(&artifact.path)) || outputs.iter().any(|(_, directory)| !matches(directory)) {
            return Err(Error::Settings("captured artifacts do not match the producer target/profile output directories".into()));
        }
        let provided = artifacts.iter().map(|artifact| artifact.producer.as_str()).collect::<BTreeSet<_>>();
        if packages.values().any(|name| !provided.contains(name.as_str())) {
            return Err(Error::Settings("captured build is missing a selected package artifact".into()));
        }
        if provided.len() != artifacts.len() {
            return Err(Error::Settings("captured build contains ambiguous package artifacts".into()));
        }
        for artifact in &artifacts {
            if receipt.artifacts.get(&artifact.path) != Some(&artifact.sha256) {
                return Err(Error::ArtifactChanged(artifact.path.clone()));
            }
        }
        finish_build(
            settings.backend,
            receipt.profile,
            receipt.target,
            artifacts,
            outputs,
            &BuildCancellation::default(),
            Some(&receipt.generated),
        )
    }

    // Relocation Changes Locations Only; It Never Replaces The Original Byte Stamps.
    pub fn relocate(&mut self, from: &Path, to: &Path) -> Result<(), Error> {
        let relocate = |path: &Path| {
            path.strip_prefix(from)
                .map(|relative| to.join(relative))
                .map_err(|_| Error::Settings("build receipt paths do not share the selected source root".into()))
        };
        let artifacts = self.artifacts.iter().map(|artifact| relocate(&artifact.path)).collect::<Result<Vec<_>, _>>()?;
        let schemas = self.schemas.iter().map(|schema| relocate(&schema.directory)).collect::<Result<Vec<_>, _>>()?;
        for (artifact, path) in self.artifacts.iter_mut().zip(artifacts) {
            artifact.path = path;
        }
        for (schema, directory) in self.schemas.iter_mut().zip(schemas) {
            schema.directory = directory;
        }
        Ok(())
    }
}

/* === Public Functions === */

pub fn build(settings: &BuildSettings) -> Result<Build, Error> {
    build_with_cancellation(settings, &BuildCancellation::default())
}

pub fn build_with_cancellation(settings: &BuildSettings, cancellation: &BuildCancellation) -> Result<Build, Error> {
    validate_backend(settings)?;
    let metadata = Metadata::read(settings, cancellation)?;
    build_resolved(settings, cancellation, metadata)
}

pub(super) fn build_resolved(settings: &BuildSettings, cancellation: &BuildCancellation, metadata: Metadata) -> Result<Build, Error> {
    let manifest = settings.manifest.canonicalize()?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let directory = manifest.parent().ok_or_else(|| Error::Settings("manifest has no project directory".into()))?;
    let selected = metadata.select(settings)?;
    let target = match (&settings.backend, &settings.target) {
        (BuildBackend::Guest, None) => "wasm32-unknown-unknown".to_owned(),
        (_, Some(target)) => target.clone(),
        (BuildBackend::Sidecar, None) => {
            let mut command = Command::new("rustc");
            command.args(["-vV"]).stdout(Stdio::piped()).stderr(Stdio::piped());
            let version = Process::spawn(command, cancellation)?.output()?;
            cancellation.check()?;
            if !version.status.success() {
                return Err(Error::Cargo("rustc host discovery failed".into()));
            }
            String::from_utf8_lossy(&version.stdout)
                .lines()
                .find_map(|line| line.strip_prefix("host: "))
                .ok_or_else(|| Error::Cargo("rustc did not report its host target".into()))?
                .to_owned()
        }
    };
    if (settings.backend == BuildBackend::Guest) != target.starts_with("wasm32-") {
        return Err(Error::Settings("backend and selected target disagree".into()));
    }
    let mut command = Command::new(cargo);
    command.current_dir(directory);
    command.args(["build", "--locked", "--message-format=json-render-diagnostics", "--manifest-path"]).arg(&manifest);
    for package in selected.values() {
        command.args(["--package", package]);
    }
    command.args(["--profile", settings.profile.name()]);
    if settings.timings {
        command.arg("--timings");
    }
    command.args(["--target", &target]);
    if let Some(directory) = &settings.target_directory {
        command.arg("--target-dir").arg(directory);
    }
    if !settings.default_features {
        command.arg("--no-default-features");
    }
    if !settings.features.is_empty() {
        command.args(["--features", &settings.features.join(",")]);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::inherit());
    let mut child = Process::spawn(command, cancellation)?;
    let parsed = read_messages(BufReader::new(child.stdout()), &selected);
    if parsed.is_err() {
        child.kill()?;
    }
    let status = child.wait()?;
    cancellation.check()?;
    let (artifacts, outputs) = parsed?;
    if !status.success() {
        return Err(Error::Cargo(status.to_string()));
    }

    finish_build(settings.backend, settings.profile, target, artifacts, outputs, cancellation, None)
}

fn finish_build(
    backend: BuildBackend, profile: BuildProfile, target: String, mut artifacts: Vec<Artifact>, outputs: BTreeSet<(String, PathBuf)>,
    cancellation: &BuildCancellation, generated: Option<&BTreeMap<PathBuf, String>>,
) -> Result<Build, Error> {
    artifacts
        .retain(|artifact| (artifact.path.extension().is_some_and(|extension| extension == "wasm")) == (backend == BuildBackend::Guest));
    if artifacts.is_empty() {
        return Err(Error::Settings("selected packages contain no artifact for this backend".into()));
    }
    #[cfg(feature = "guest")]
    let mut components: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    #[cfg(not(feature = "guest"))]
    let components: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    // Cargo Emits Core Modules; Consumers Receive Ready-To-Load Components.
    #[cfg(feature = "guest")]
    for artifact in &mut artifacts {
        if artifact.path.extension().is_none_or(|extension| extension != "wasm") {
            continue;
        }
        let module = std::fs::read(&artifact.path)?;
        if digest(&module) != artifact.sha256 {
            return Err(Error::ArtifactChanged(artifact.path.clone()));
        }
        super::Declaration::inspect(&module, super::Facility::Guest)?;
        if module.starts_with(b"\0asm\x0d\0\x01\0") {
            continue;
        }
        let component = super::componentize(&module).map_err(Error::Component)?;
        super::Declaration::inspect(&component, super::Facility::Guest)?;
        let output = artifact.path.with_extension("component.wasm");
        artifact.sha256 = digest(&component);
        components.insert(output.clone(), component);
        artifact.path = output;
    }

    let expected_schemas = generated.map(|generated| {
        generated
            .iter()
            .filter(|(path, _)| outputs.iter().any(|(_, directory)| path.starts_with(directory.join("grpc"))))
            .map(|(path, sha256)| (path.clone(), sha256.clone()))
            .collect::<BTreeMap<_, _>>()
    });
    let mut schemas = BTreeMap::new();
    for (producer, directory) in outputs {
        let root = directory.join("grpc");
        if !root.exists() {
            continue;
        }
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let catalog = entry.file_name().into_string().map_err(|_| Error::Settings("schema catalog is not UTF-8".into()))?;
            let key = (producer.clone(), catalog.clone());
            if schemas
                .insert(
                    key.clone(),
                    Schema {
                        producer: producer.clone(),
                        catalog,
                        files: schema_files(&entry.path())?.into_iter().map(|(name, bytes)| (name, digest(&bytes))).collect(),
                        directory: entry.path(),
                    },
                )
                .is_some()
            {
                return Err(Error::Collision(format!("{}/{}", key.0, key.1)));
            }
        }
    }
    let schemas: Vec<_> = schemas.into_values().collect();
    if let Some(expected) = expected_schemas {
        let actual = schemas
            .iter()
            .flat_map(|schema| schema.files.iter().map(|(name, sha256)| (schema.directory.join(name), sha256.clone())))
            .collect::<BTreeMap<_, _>>();
        if actual != expected {
            return Err(Error::Settings("generated schema set differs from the archive producer snapshot".into()));
        }
    }
    cancellation.check()?;
    for (path, bytes) in components {
        publish(&path, &bytes)?;
    }
    Ok(Build { backend, profile, target, artifacts, schemas })
}

/* === Private Functions === */

pub(super) fn validate_backend(settings: &BuildSettings) -> Result<(), Error> {
    let enabled = match settings.backend {
        BuildBackend::Sidecar => cfg!(feature = "sidecar"),
        BuildBackend::Guest => cfg!(feature = "guest"),
    };
    if !enabled {
        return Err(Error::Settings("selected packaging backend is disabled".into()));
    }
    Ok(())
}

// Package Only The Exact Bytes Stamped By An Earlier Build. This Path Never Runs Cargo.
pub fn package(build: &Build, output: &Path) -> Result<Vec<PackageOutput>, Error> {
    package_with_cancellation(build, output, &BuildCancellation::default())
}

pub fn package_with_cancellation(build: &Build, output: &Path, cancellation: &BuildCancellation) -> Result<Vec<PackageOutput>, Error> {
    if build.artifacts.is_empty() || build.target.is_empty() {
        return Err(Error::Settings("a stamped build with artifacts and a target is required".into()));
    }
    let artifacts = &build.artifacts;
    let schemas = &build.schemas;
    let backend = build.backend;
    let target = &build.target;
    use crate::bundle::{Bundle, Executable, Facility, Identity, Limits, Source};
    let facility = match backend {
        BuildBackend::Sidecar => Facility::Sidecar,
        BuildBackend::Guest => Facility::Guest,
    };
    let inspected = match backend {
        BuildBackend::Sidecar => super::Facility::Sidecar,
        BuildBackend::Guest => super::Facility::Guest,
    };
    let mut prepared = BTreeMap::new();
    for artifact in artifacts {
        cancellation.check()?;
        safe_name(&artifact.producer)?;
        safe_name(&artifact.name)?;
        let content = read_artifact(&artifact.path)?;
        if digest(&content) != artifact.sha256 {
            return Err(Error::ArtifactChanged(artifact.path.clone()));
        }
        let declaration = super::Declaration::inspect(&content, inspected)?;
        let executable = artifact
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Settings("artifact filename is not UTF-8".into()))?
            .to_owned();
        let mut files = BTreeMap::from([(executable.clone(), content)]);
        for schema in schemas.iter().filter(|schema| schema.producer == artifact.producer) {
            safe_name(&schema.catalog)?;
            let contents = schema_files(&schema.directory)?;
            let hashes: BTreeMap<_, _> = contents.iter().map(|(name, bytes)| (name.clone(), digest(bytes))).collect();
            if hashes != schema.files {
                return Err(Error::ArtifactChanged(schema.directory.clone()));
            }
            for (name, bytes) in contents {
                let name = format!("grpc/{}/{name}", schema.catalog);
                if files.insert(name.clone(), bytes).is_some() {
                    return Err(Error::Collision(name));
                }
            }
        }
        let sources: Vec<_> = files
            .iter()
            .map(|(path, bytes)| Source {
                path,
                bytes,
                executable: (path == &executable).then(|| Executable { facility, target: target.to_owned(), registrations: Vec::new() }),
            })
            .collect();
        let identity = Identity {
            name: declaration.id,
            version: declaration.version,
            description: declaration.description,
            package: Default::default(),
        };
        let bundle = Bundle::build(identity, &sources, Limits::default())?;
        let destination = output.join(&artifact.producer).join(format!("{}.harmos", artifact.name));
        if prepared.insert(destination.clone(), (artifact.producer.clone(), bundle)).is_some() {
            return Err(Error::Collision(destination.display().to_string()));
        }
    }
    cancellation.check()?;
    // Publish Only After Every Artifact And Schema Has Passed Preparation.
    let mut packages = Vec::new();
    for (path, (producer, bundle)) in prepared {
        let sha256 = digest(bundle.bytes());
        publish(&path, bundle.bytes())?;
        packages.push(PackageOutput { producer, path, sha256 });
    }
    Ok(packages)
}

fn safe_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name.len() > 255
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        || matches!(name, "." | "..")
    {
        return Err(Error::Settings("artifact names must be single path components".into()));
    }
    Ok(())
}

fn read_artifact(path: &Path) -> Result<Vec<u8>, Error> {
    use std::io::Read as _;
    let limit = crate::bundle::Limits::default().file_bytes;
    if path.is_symlink() {
        return Err(Error::Settings("artifact must be a regular file".into()));
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Settings("artifact must be a regular file".into()));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error::Settings("artifact exceeds bundle byte limit".into()));
    }
    Ok(bytes)
}

fn schema_files(directory: &Path) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let manifest = read_artifact(&directory.join("manifest.txt"))?;
    let text = std::str::from_utf8(&manifest).map_err(|_| Error::Settings("schema manifest must be UTF-8".into()))?;
    let names: Vec<_> = text.lines().collect();
    if names.first() != Some(&"descriptors.bin")
        || names.len() > crate::bundle::Limits::default().files
        || names.iter().copied().collect::<BTreeSet<_>>().len() != names.len()
        || names.contains(&"manifest.txt")
    {
        return Err(Error::Settings("schema manifest is incomplete or duplicated".into()));
    }
    let mut files = BTreeMap::new();
    for name in names {
        let relative = Path::new(name);
        if relative.components().any(|part| !matches!(part, std::path::Component::Normal(_))) || name.contains(['\\', ':']) {
            return Err(Error::Settings("schema manifest contains an unsafe path".into()));
        }
        files.insert(name.to_owned(), read_artifact(&directory.join(relative))?);
    }
    files.insert("manifest.txt".into(), manifest);
    Ok(files)
}

fn publish(path: &std::path::Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    let parent = path.parent().ok_or_else(|| Error::Settings("output has no parent directory".into()))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.persist(path).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn read_messages(reader: impl BufRead, selected: &BTreeMap<String, String>) -> Result<BuildMessages, Error> {
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
            eprint!("{rendered}");
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
        let files = message
            .executable
            .into_iter()
            .chain(message.filenames.into_iter().filter(|path| path.extension().is_some_and(|extension| extension == "wasm")));
        for path in files {
            artifacts.push(Artifact::read(producer.clone(), target.name.clone(), path)?);
        }
    }
    if !finished {
        return Err(Error::Cargo("captured Cargo stream has no successful build-finished event".into()));
    }
    artifacts.sort_by(|left, right| (&left.producer, &left.name, &left.path).cmp(&(&right.producer, &right.name, &right.path)));
    artifacts.dedup_by(|left, right| left.producer == right.producer && left.name == right.name && left.path == right.path);
    Ok((artifacts, outputs))
}

/* === Tests === */
