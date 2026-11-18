/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::path::{Path, PathBuf};

// Third Party Imports
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, de};

// Local Imports
use harmos_operations::Registration;

use super::{Error, invalid};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub name: String,
    pub version: Version,
    pub description: String,

    #[serde(default)]
    pub package: PackageMetadata,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMetadata {
    pub authors: Vec<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub identity: Identity,
    pub files: Vec<File>,

    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "decode_launch")]
    pub launch: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub path: String,
    pub size: u64,
    pub sha256: String,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub executable: Option<Executable>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executable {
    pub facility: Facility,
    pub target: String,
    pub registrations: Vec<Registration>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub archive_bytes: usize,
    pub manifest_bytes: usize,
    pub file_bytes: usize,
    pub total_file_bytes: usize,
    pub files: usize,
    pub registrations: usize,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Facility {
    Sidecar,
    Guest,
}

/* === Traits === */

/* === Implementations === */

impl Manifest {
    /// Whether Extraction Must Preserve Executable Permissions for This File.
    pub fn is_executable(&self, file: &File) -> bool {
        let native = file.executable.as_ref().is_some_and(|value| value.facility == Facility::Sidecar);
        let launched = self
            .launch
            .as_ref()
            .and_then(|launch| launch.first())
            .and_then(|program| relative_program(program).ok().flatten())
            .is_some_and(|path| path == Path::new(&file.path));
        native || launched
    }
}

impl File {
    pub(crate) fn verify(&self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() as u64 != self.size || super::digest(bytes) != self.sha256 {
            return Err(invalid("file size or digest mismatch"));
        }
        Ok(())
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            archive_bytes: 272 * 1024 * 1024,
            manifest_bytes: 16 * 1024 * 1024,
            file_bytes: 128 * 1024 * 1024,
            total_file_bytes: 256 * 1024 * 1024,
            files: 4096,
            registrations: 1024,
        }
    }
}

/* === Public Functions === */

/// Validate a Command Without Resolving Programs on the Build Machine.
pub fn validate_launch(launch: &[String]) -> Result<(), Error> {
    let program = launch.first().filter(|program| !program.is_empty()).ok_or_else(|| invalid("launch must contain a nonempty program"))?;
    if launch.iter().any(|argument| argument.contains('\0')) {
        return Err(invalid("launch strings must not contain NUL"));
    }
    relative_program(program)?;
    Ok(())
}

/* === Private Functions === */

fn decode_launch<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<String>>, D::Error> {
    let launch = Vec::<String>::deserialize(deserializer)?;
    validate_launch(&launch).map_err(de::Error::custom)?;
    Ok(Some(launch))
}

pub(crate) fn relative_program(program: &str) -> Result<Option<PathBuf>, Error> {
    if program.starts_with(['/', '\\']) || program.contains(':') {
        return Err(invalid("launch program must be on PATH or relative to the bundle root; remove the absolute path"));
    }
    if !program.contains(['/', '\\']) && program != "." && program != ".." {
        return Ok(None);
    }
    let mut path = PathBuf::new();
    for component in program.split(['/', '\\']) {
        match component {
            "" | "." => {}
            ".." => {
                if !path.pop() {
                    return Err(invalid("launch program escapes the bundle root; use a path inside the bundle"));
                }
            }
            _ => path.push(component),
        }
    }
    if path.as_os_str().is_empty() {
        return Err(invalid("launch program must name a program inside the bundle or on PATH"));
    }
    Ok(Some(path))
}

/* === Tests === */
