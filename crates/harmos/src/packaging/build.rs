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
use std::path::PathBuf;
use std::process::{Command, Stdio};

// Third Party Imports
use serde::Deserialize;

// Local Imports
use super::{BuildCancellation, Error, metadata::Metadata, process::Process};
use crate::config::{BuildBackend, BuildSettings};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Artifact {
    pub producer: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Schema {
    pub producer: String,
    pub catalog: String,
    pub directory: PathBuf,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Build {
    pub artifacts: Vec<Artifact>,
    pub schemas: Vec<Schema>,
    pub packages: Vec<PackageOutput>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageOutput {
    pub producer: String,
    pub path: PathBuf,
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
}

#[derive(Deserialize)]
struct Diagnostic {
    rendered: Option<String>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn build(settings: &BuildSettings) -> Result<Build, Error> {
    build_with_cancellation(settings, &BuildCancellation::default())
}

pub fn build_with_cancellation(
    settings: &BuildSettings,
    cancellation: &BuildCancellation,
) -> Result<Build, Error> {
    validate_backend(settings)?;
    let metadata = Metadata::read(settings, cancellation)?;
    build_resolved(settings, cancellation, metadata)
}

pub(super) fn build_resolved(
    settings: &BuildSettings,
    cancellation: &BuildCancellation,
    metadata: Metadata,
) -> Result<Build, Error> {
    let manifest = settings.manifest.canonicalize()?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let directory = manifest
        .parent()
        .ok_or_else(|| Error::Settings("manifest has no project directory".into()))?;
    let selected = metadata.select(settings)?;
    let target = match (&settings.backend, &settings.target) {
        (BuildBackend::Guest, None) => "wasm32-unknown-unknown".to_owned(),
        (_, Some(target)) => target.clone(),
        (BuildBackend::Sidecar, None) => {
            let mut command = Command::new("rustc");
            command
                .args(["-vV"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
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
        return Err(Error::Settings(
            "backend and selected target disagree".into(),
        ));
    }
    let mut command = Command::new(cargo);
    command.current_dir(directory);
    command
        .args([
            "build",
            "--locked",
            "--message-format=json-render-diagnostics",
            "--manifest-path",
        ])
        .arg(&manifest);
    for package in selected.values() {
        command.args(["--package", package]);
    }
    if settings.release {
        command.arg("--release");
    }
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
    let (mut artifacts, outputs) = parsed?;
    if !status.success() {
        return Err(Error::Cargo(status.to_string()));
    }

    artifacts.retain(|artifact| {
        (artifact
            .path
            .extension()
            .is_some_and(|extension| extension == "wasm"))
            == (settings.backend == BuildBackend::Guest)
    });
    if artifacts.is_empty() {
        return Err(Error::Settings(
            "selected packages contain no artifact for this backend".into(),
        ));
    }
    #[cfg(feature = "packaging-guest")]
    let mut components = BTreeMap::new();
    #[cfg(not(feature = "packaging-guest"))]
    let components = BTreeMap::new();
    // Cargo Emits Core Modules; Consumers Receive Ready-To-Load Components.
    #[cfg(feature = "packaging-guest")]
    for artifact in &mut artifacts {
        if artifact
            .path
            .extension()
            .is_none_or(|extension| extension != "wasm")
        {
            continue;
        }
        let module = std::fs::read(&artifact.path)?;
        super::Declaration::inspect(&module, super::Facility::Guest)?;
        if module.starts_with(b"\0asm\x0d\0\x01\0") {
            continue;
        }
        let component = super::componentize(&module).map_err(Error::Component)?;
        super::Declaration::inspect(&component, super::Facility::Guest)?;
        let output = artifact.path.with_extension("component.wasm");
        components.insert(output.clone(), component);
        artifact.path = output;
    }

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
            let catalog = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::Settings("schema catalog is not UTF-8".into()))?;
            let key = (producer.clone(), catalog.clone());
            if schemas
                .insert(
                    key.clone(),
                    Schema {
                        producer: producer.clone(),
                        catalog,
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
    let output = metadata
        .output_directory(settings)?
        .join("harmos")
        .join(&target);
    let packages = package(
        &artifacts,
        &schemas,
        settings.backend,
        &target,
        &output,
        components,
        cancellation,
    )?;
    Ok(Build {
        artifacts,
        schemas,
        packages,
    })
}

/* === Private Functions === */
pub(super) fn validate_backend(settings: &BuildSettings) -> Result<(), Error> {
    let enabled = match settings.backend {
        BuildBackend::Sidecar => cfg!(feature = "packaging-sidecar"),
        BuildBackend::Guest => cfg!(feature = "packaging-guest"),
    };
    if !enabled {
        return Err(Error::Settings(
            "selected packaging backend is disabled".into(),
        ));
    }
    Ok(())
}

fn package(
    artifacts: &[Artifact],
    schemas: &[Schema],
    backend: BuildBackend,
    target: &str,
    output: &std::path::Path,
    components: BTreeMap<PathBuf, Vec<u8>>,
    cancellation: &BuildCancellation,
) -> Result<Vec<PackageOutput>, Error> {
    use super::bundle::{Bundle, Executable, Facility, Identity, Limits, Source};
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
        let content = match components.get(&artifact.path) {
            Some(bytes) => bytes.clone(),
            None => std::fs::read(&artifact.path)?,
        };
        let declaration = super::Declaration::inspect(&content, inspected)?;
        let executable = artifact
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Settings("artifact filename is not UTF-8".into()))?
            .to_owned();
        let mut files = BTreeMap::from([(executable.clone(), content)]);
        for schema in schemas
            .iter()
            .filter(|schema| schema.producer == artifact.producer)
        {
            let manifest = std::fs::read_to_string(schema.directory.join("manifest.txt"))?;
            let names: Vec<_> = manifest.lines().collect();
            if names.first() != Some(&"descriptors.bin")
                || names.iter().copied().collect::<BTreeSet<_>>().len() != names.len()
            {
                return Err(Error::Settings(
                    "schema manifest is incomplete or duplicated".into(),
                ));
            }
            for name in names.into_iter().chain(["manifest.txt"]) {
                let relative = std::path::Path::new(name);
                if relative
                    .components()
                    .any(|component| !matches!(component, std::path::Component::Normal(_)))
                    || name.contains(['\\', ':'])
                {
                    return Err(Error::Settings(
                        "schema manifest contains an unsafe path".into(),
                    ));
                }
                let path = schema.directory.join(relative);
                if path.is_symlink() || !path.is_file() {
                    return Err(Error::Settings("schema must be a regular file".into()));
                }
                let name = format!("grpc/{}/{name}", schema.catalog);
                if files.insert(name.clone(), std::fs::read(path)?).is_some() {
                    return Err(Error::Collision(name));
                }
            }
        }
        let sources: Vec<_> = files
            .iter()
            .map(|(path, bytes)| Source {
                path,
                bytes,
                executable: (path == &executable).then(|| Executable {
                    facility,
                    target: target.to_owned(),
                    registrations: Vec::new(),
                }),
            })
            .collect();
        let identity = Identity {
            name: declaration.id,
            version: declaration.version,
            description: declaration.description,
            package: Default::default(),
        };
        let bundle = Bundle::build(identity, &sources, Limits::default())?;
        let destination = output
            .join(&artifact.producer)
            .join(format!("{}.harmos", artifact.name));
        if prepared
            .insert(destination.clone(), (artifact.producer.clone(), bundle))
            .is_some()
        {
            return Err(Error::Collision(destination.display().to_string()));
        }
    }
    cancellation.check()?;
    // Publish Only After Every Artifact And Schema Has Passed Preparation.
    for (path, bytes) in components {
        publish(&path, &bytes)?;
    }
    let mut packages = Vec::new();
    for (path, (producer, bundle)) in prepared {
        publish(&path, bundle.bytes())?;
        packages.push(PackageOutput { producer, path });
    }
    Ok(packages)
}

fn publish(path: &std::path::Path, bytes: &[u8]) -> Result<(), Error> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| Error::Settings("output has no parent directory".into()))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary
        .persist(path)
        .map_err(|error| Error::Io(error.error))?;
    Ok(())
}

fn read_messages(
    reader: impl BufRead,
    selected: &BTreeMap<String, String>,
) -> Result<(Vec<Artifact>, BTreeSet<(String, PathBuf)>), Error> {
    let mut artifacts = Vec::new();
    let mut outputs = BTreeSet::new();
    for line in reader.lines() {
        let message: Message = serde_json::from_str(&line?)?;
        if let Some(rendered) = message
            .message
            .as_ref()
            .and_then(|diagnostic| diagnostic.rendered.as_ref())
        {
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
        if message.reason != "compiler-artifact" {
            continue;
        }
        let Some(target) = message.target else {
            continue;
        };
        if target.kind.iter().any(|kind| {
            kind == "custom-build" || kind == "example" || kind == "test" || kind == "bench"
        }) {
            continue;
        }
        let files = message
            .executable
            .into_iter()
            .chain(message.filenames.into_iter().filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "wasm")
            }));
        for path in files {
            artifacts.push(Artifact {
                producer: producer.clone(),
                name: target.name.clone(),
                path,
            });
        }
    }
    Ok((artifacts, outputs))
}

/* === Tests === */
