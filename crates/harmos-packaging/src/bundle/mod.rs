/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod archive;
mod error;
mod manifest;
mod metadata;

pub use error::Error;
pub use manifest::{Executable, Facility, File, Identity, Limits, Manifest, PackageMetadata};

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::ops::Range;
use std::path::Path;

use harmos_domain::inventory::Declaration;

pub struct Source<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
    pub executable: Option<Executable>,
}

pub struct Bundle {
    manifest: Manifest,
    bytes: Vec<u8>,
    contents: BTreeMap<String, Range<usize>>,
    declarations: BTreeMap<String, Declaration>,
}

impl Bundle {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn build(identity: Identity, sources: &[Source<'_>], limits: Limits) -> Result<Self, Error> {
        if sources.len() > limits.files {
            return Err(Error::Limit("file count"));
        }
        let mut files = Vec::with_capacity(sources.len());
        let mut contents = BTreeMap::new();
        let mut total = 0usize;
        let mut metadata_bytes = metadata::identity_size(&identity, limits.manifest_bytes)?;
        for source in sources {
            archive::path(source.path)?;
            total = total.checked_add(source.bytes.len()).ok_or(Error::Limit("total file bytes"))?;
            if source.bytes.len() > limits.file_bytes || total > limits.total_file_bytes {
                return Err(Error::Limit("file bytes"));
            }
            if contents.insert(source.path.to_owned(), source.bytes).is_some() {
                return Err(invalid("duplicate file path"));
            }
            if source.executable.as_ref().is_some_and(|value| value.registrations.len() > limits.registrations) {
                return Err(Error::Limit("registration count"));
            }
            let sha256 = crate::digest(source.bytes);
            metadata::file_size(source, &sha256, !files.is_empty(), &mut metadata_bytes, limits.manifest_bytes)?;
            let mut executable = source.executable.clone();
            if let Some(executable) = &mut executable {
                executable.registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
            }
            files.push(File {
                path: source.path.to_owned(),
                size: source.bytes.len() as u64,
                sha256,
                executable,
            });
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = Manifest { schema: 1, identity, files };
        validate(&manifest, limits)?;
        let bytes = archive::encode(&manifest, &contents, limits)?;
        Self::inspect(&bytes, limits)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn inspect(bytes: &[u8], limits: Limits) -> Result<Self, Error> {
        let (manifest, contents) = archive::decode(bytes, limits)?;
        let mut declarations = BTreeMap::new();
        let mut owners = BTreeSet::new();
        for file in &manifest.files {
            let content = &bytes[contents[&file.path].clone()];
            if let Some(executable) = &file.executable {
                let facility = match executable.facility {
                    Facility::Sidecar => harmos_domain::inventory::Facility::Sidecar,
                    Facility::Guest => harmos_domain::inventory::Facility::Guest,
                };
                let declaration = Declaration::inspect(content, facility).map_err(|error| invalid(error.to_string()))?;
                if !owners.insert((executable.facility, declaration.id.clone(), &executable.target)) {
                    return Err(invalid("duplicate executable facility, owner, and target"));
                }
                declarations.insert(file.path.clone(), declaration);
            }
        }
        Ok(Self {
            manifest,
            bytes: bytes.to_vec(),
            contents,
            declarations,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn declaration(&self, path: &str) -> Option<&Declaration> {
        self.declarations.get(path)
    }

    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.contents.get(path).map(|range| &self.bytes[range.clone()])
    }

    pub fn extract(&self, parent: &Path) -> Result<tempfile::TempDir, Error> {
        let directory = archive::staging(parent)?;
        for file in &self.manifest.files {
            let path = directory.path().join(&file.path);
            std::fs::create_dir_all(path.parent().expect("a file has a staging parent"))?;
            let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
            output.write_all(self.file(&file.path).expect("verified file exists"))?;
            archive::permissions(&output, file.executable.as_ref().is_some_and(|value| value.facility == Facility::Sidecar))?;
        }
        let path = directory.path().join("manifest.json");
        let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        serde_json::to_writer(&mut output, &self.manifest)?;
        archive::permissions(&output, false)?;
        Ok(directory)
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

fn validate(manifest: &Manifest, limits: Limits) -> Result<(), Error> {
    if manifest.schema != 1 {
        return Err(invalid("unsupported schema"));
    }
    let name = &manifest.identity.name;
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')) {
        return Err(invalid("invalid Cargo package name"));
    }
    if manifest.files.len() > limits.files {
        return Err(Error::Limit("file count"));
    }
    let mut paths = BTreeSet::new();
    let mut previous = None;
    for file in &manifest.files {
        archive::path(&file.path)?;
        if previous.is_some_and(|value: &str| value >= file.path.as_str()) {
            return Err(invalid("files are not in unique path order"));
        }
        previous = Some(&file.path);
        if !paths.insert(file.path.to_ascii_lowercase()) {
            return Err(invalid("case-insensitive file collision"));
        }
        if let Some(executable) = &file.executable {
            if executable.target.is_empty()
                || executable.target.len() > 128
                || !executable.target.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            {
                return Err(invalid("invalid executable target"));
            }
            if executable.registrations.len() > limits.registrations {
                return Err(Error::Limit("registration count"));
            }
            let mut previous = None;
            for registration in &executable.registrations {
                if !harmos_domain::artifact_subset(&registration.namespace)
                    || registration.schema == 0
                    || previous.is_some_and(|value: &str| value >= registration.namespace.as_str())
                {
                    return Err(invalid("invalid, duplicate, or unordered registration"));
                }
                previous = Some(&registration.namespace);
                serde_json::from_slice::<&serde_json::value::RawValue>(&registration.payload)?;
            }
        }
    }
    for path in &paths {
        for (offset, _) in path.match_indices('/') {
            if paths.contains(&path[..offset]) {
                return Err(invalid("file path is another file's parent"));
            }
        }
    }
    Ok(())
}
