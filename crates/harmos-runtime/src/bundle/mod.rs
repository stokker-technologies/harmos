/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod archive;
mod error;
mod guest;
mod manifest;
mod metadata;

/* === Re-Exports === */

pub use archive::path as validate_path;
pub use error::Error;
pub use guest::{METADATA_SECTION as GUEST_METADATA_SECTION, registration as guest_registration};
pub use manifest::{Executable, Facility, File, Identity, Limits, Manifest, PackageMetadata};

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::ops::Range;
use std::path::Path;
// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use harmos_protocol::v1::GuestRegistration;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Bundle {
    manifest: Manifest,
    bytes: Vec<u8>,
    contents: BTreeMap<String, Range<usize>>,
    declarations: BTreeMap<String, GuestRegistration>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Bundle {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn inspect(bytes: &[u8], limits: Limits) -> Result<Self, Error> {
        let (manifest, contents) = archive::decode(bytes, limits)?;
        let mut declarations = BTreeMap::new();
        let mut owners = BTreeSet::new();
        for file in &manifest.files {
            let content = &bytes[contents[&file.path].clone()];
            if let Some(executable) = &file.executable {
                if executable.facility == Facility::Sidecar {
                    if !executable.registrations.is_empty() {
                        return Err(invalid("native catalogs are obtained through Plugin.Get"));
                    }
                    if !owners.insert((executable.facility, manifest.identity.name.clone(), &executable.target)) {
                        return Err(invalid("duplicate executable facility, owner, and target"));
                    }
                    continue;
                }
                let declaration = guest_registration(content)?;
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

    pub fn declaration(&self, path: &str) -> Option<&GuestRegistration> {
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

/* === Public Functions === */

pub fn digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let hash = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(hash.len() * 2);
    for byte in hash {
        let high = char::from(HEX[usize::from(byte >> 4)]);
        let low = char::from(HEX[usize::from(byte & 0x0f)]);
        encoded.push(high);
        encoded.push(low);
    }
    encoded
}

/* === Private Functions === */

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

fn validate(manifest: &Manifest, limits: Limits) -> Result<(), Error> {
    if manifest.schema != 1 {
        return Err(invalid("unsupported schema"));
    }
    let name = &manifest.identity.name;
    let native =
        manifest.files.iter().any(|file| file.executable.as_ref().is_some_and(|executable| executable.facility == Facility::Sidecar));
    if name.is_empty() || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') || (native && byte == b'.'))
    {
        return Err(invalid("invalid plugin identity"));
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

/* === Tests === */
